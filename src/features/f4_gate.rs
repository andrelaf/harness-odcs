//! F4 — Gate + relatorio de lacunas: o contrato e enriquecido com a
//! classificacao de F3, o que ficou sem decisao vai para o relatorio, e nada e
//! persistido enquanto houver pendencia que o harness nao tem autoridade para
//! resolver sozinho.
//!
//! Spec da feature: `docs/spec-f4-gate.md`.
//!
//! **O contrato e a unica fonte da decisao anterior.** O harness nao guarda "o
//! que eu classifiquei da ultima vez": le o que o contrato declara hoje —
//! `classification` e `tags: [pii, sensitive]` — e compara com o que o catalogo
//! diz agora. O que esta no arquivo e a decisao humana vigente, porque foi ela
//! que passou por commit e revisao.
//!
//! A consequencia e o que faz o gate funcionar sem ninguem avisa-lo: mudar a
//! classificacao de um termo no catalogo aparece sozinha, no run seguinte, como
//! divergencia entre a proposta e o contrato.
//!
//! Campo sem nada declarado **nao** abre gate: e primeira classificacao, nao
//! reclassificacao. Gate que sempre dispara e gate que ninguem le.

use crate::features::contrato;
use crate::features::f1_validar;
use crate::features::f2_mapear::Mapeamento;
use crate::features::f3_classificar::{
    self, CampoClassificado, Catalogo, Laudo, Situacao, niveis_legivel, sim_nao,
};
use crate::flow::Outcome;
use crate::phases::Run;
use crate::state::{Aprovacoes, GatePendente, SCHEMA_VERSION};
use crate::tools;
use crate::trace;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_norway::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

/// A feature a que as aprovacoes se referem. A aprovacao casa feature **e**
/// hash — nenhuma das duas sozinha identifica o que foi liberado.
const FEATURE: &str = "f4-gate";

/// De onde o vinculo com o glossario aponta, dentro de
/// `authoritativeDefinitions`. Caminho no repositorio, e nao URL de servidor:
/// o glossario que classificou este campo esta versionado ao lado do contrato,
/// e e ele que responde a auditoria.
const URL_GLOSSARIO: &str = "glossary/glossario.yaml";

/// O `type` que o ODCS 3.1.0 usa para vinculo com definicao de negocio.
const TIPO_DEFINICAO: &str = "businessDefinition";

/// A ausencia, dita de cada lado da comparacao. O contrato nao declarou; o
/// catalogo nao classificou. Sao coisas diferentes e a tabela do relatorio
/// coloca as duas lado a lado.
const NADA_DECLARADO: &str = "nada declarado";
const SEM_CLASSIFICACAO: &str = "sem classificacao";

// --- Fases -------------------------------------------------------------------

/// Propoe o enriquecimento e submete ao gate o que exigir decisao humana.
pub fn implement(run: &mut Run) -> Outcome {
    let c = match compor(run, "implement") {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(e),
    };

    // A evidencia e gravada **antes** da decisao do gate, de proposito: quem
    // aprova precisa poder ler a proposta inteira, nao so a lista de itens.
    if let Err(e) = gravar_proposta(run, &c) {
        return Outcome::Fail(e);
    }

    if c.proposta.gate.is_empty() {
        run.note("nenhuma pendencia — nada a submeter ao gate".to_string());
        return Outcome::Pass;
    }

    let aprovacoes = match Aprovacoes::load_or_default(&run.cfg.aprovacoes_path()) {
        Ok(a) => a,
        Err(e) => return Outcome::Fail(format!("lendo o livro de aprovacoes: {e:#}")),
    };

    match aprovacoes.cobrindo(FEATURE, &c.proposta.gate_sha256) {
        Some(a) => {
            run.note(format!(
                "gate liberado por decisao humana em {} — pedido {} ({})",
                a.aprovado_em,
                &a.gate_sha256[..16],
                a.resumo
            ));
            Outcome::Pass
        }
        None => bloquear(run, &c.proposta),
    }
}

/// Refaz tudo do zero, julga, e so entao aplica o enriquecimento no contrato.
pub fn verify(run: &mut Run) -> Outcome {
    let c = match compor(run, "verify") {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(e),
    };

    // As garantias de F3 valem aqui chamadas, nao reescritas: cobertura total,
    // classificacao vinda do catalogo, resumo batendo com as decisoes.
    let mut defeitos = f3_classificar::conferir_cobertura(&c.mapeamento, &c.laudo, &c.catalogo);

    // Classificacao que nao achou onde ser escrita e decisao perdida em
    // silencio — o oposto do que o harness existe para garantir.
    for campo in &c.nao_aplicados {
        defeitos.push(format!(
            "campo `{campo}` foi classificado mas nao existe como propriedade no contrato — \
             o enriquecimento nao teria onde ser escrito"
        ));
    }

    let aprovacoes = match Aprovacoes::load_or_default(&run.cfg.aprovacoes_path()) {
        Ok(a) => a,
        Err(e) => return Outcome::Fail(format!("lendo o livro de aprovacoes: {e:#}")),
    };
    let aprovacao = aprovacoes.cobrindo(FEATURE, &c.proposta.gate_sha256);

    // A assercao do contexto.md — "nunca decide sozinho o que e persistido" —
    // escrita como teste. Item de gate chegando aqui sem aprovacao significa
    // que `implement` deveria ter bloqueado: e defeito do harness, nao do
    // contrato.
    if !c.proposta.gate.is_empty() && aprovacao.is_none() {
        defeitos.push(format!(
            "{} item(ns) de gate sem aprovacao chegaram a `verify` — o pedido {} nao esta no \
             livro de aprovacoes",
            c.proposta.gate.len(),
            &c.proposta.gate_sha256[..16]
        ));
    }

    let conferido = match conferir_contra_implement(run, &c) {
        Ok(v) => v,
        Err(e) => {
            defeitos.push(e);
            false
        }
    };

    // A proposta recomputada e gravada por cima antes do lint: o que e julgado
    // e o que esta fase calculou, nunca um arquivo que outra fase deixou.
    if let Err(e) = gravar_proposta(run, &c) {
        return Outcome::Fail(e);
    }

    match lint_do_enriquecido(run) {
        Ok(falhas) => defeitos.extend(falhas),
        Err(e) => return Outcome::Fail(e),
    }

    let veredito = Veredito {
        aprovado: defeitos.is_empty(),
        conferido_contra_implement: conferido,
        glossario_versao: c.proposta.glossario_versao.clone(),
        catalogo_versao: c.proposta.catalogo_versao.clone(),
        resumo: c.proposta.resumo.clone(),
        gate_sha256: c.proposta.gate_sha256.clone(),
        gate: c.proposta.gate.clone(),
        aprovado_em: aprovacao.map(|a| a.aprovado_em.clone()),
        defeitos: defeitos.clone(),
    };
    if let Err(e) = gravar_veredito(run, &veredito) {
        return Outcome::Fail(format!("{e:#}"));
    }

    let r = &c.proposta.resumo;
    run.note(format!(
        "{} campo(s) — {} classificado(s) ({} primeira classificacao, {} inalterado(s)); \
         gate: {} lacuna(s), {} reclassificacao(oes), {} conflito(s)",
        r.campos,
        r.classificados,
        r.primeira_classificacao,
        r.inalterados,
        r.lacunas,
        r.reclassificacoes,
        r.conflitos
    ));

    if !defeitos.is_empty() {
        for d in &defeitos {
            run.note(format!("  defeito — {d}"));
        }
        return Outcome::Fail(defeitos.join("; "));
    }

    if let Err(e) = relatorio_legivel(run, &c, aprovacao.map(|a| a.aprovado_em.as_str())) {
        return Outcome::Fail(e);
    }

    // O que sobra depois do PASS. Campo sem classificacao nao reprova o fluxo,
    // mas quem ler o commit daqui a seis meses precisa saber que ele passou
    // sem classificacao — e por decisao de quem.
    declarar_riscos(run, &c, aprovacao.map(|a| a.aprovado_em.clone()));

    persistir(run, &c)
}

/// O que fica no repositorio depois do PASS: o contrato classificado e o laudo
/// que responde por ele.
///
/// **Contrato primeiro.** O laudo carrega o sha256 do contrato classificado,
/// entao a ordem decide como fica o repositorio se a segunda escrita falhar.
/// Contrato escrito e laudo faltando e uma ausencia — visivel, e o run seguinte
/// emite. Laudo escrito e contrato faltando seria um documento afirmando um
/// sha256 que nao esta em lugar nenhum: um laudo errado, que e pior que um
/// laudo ausente.
fn persistir(run: &mut Run, c: &Composicao) -> Outcome {
    match aplicar_no_contrato(run, c) {
        Outcome::Pass => {}
        outro => return outro,
    }
    match gravar_laudo(run, c) {
        Ok(()) => Outcome::Pass,
        Err(e) => Outcome::Fail(e),
    }
}

fn declarar_riscos(run: &mut Run, c: &Composicao, aprovado_em: Option<String>) {
    let sem_classificacao: Vec<&str> = c
        .proposta
        .campos
        .iter()
        .filter(|x| matches!(x.mudanca, Mudanca::Lacuna | Mudanca::Conflito))
        .map(|x| x.campo.as_str())
        .collect();

    if !sem_classificacao.is_empty() {
        run.risco(format!(
            "{} campo(s) persistido(s) sem classificacao: {}",
            sem_classificacao.len(),
            sem_classificacao.join(", ")
        ));
    }
    if c.proposta.resumo.reclassificacoes > 0 {
        run.risco(format!(
            "{} campo(s) tiveram a classificacao anterior sobrescrita",
            c.proposta.resumo.reclassificacoes
        ));
    }
    if let Some(ts) = aprovado_em {
        run.risco(format!(
            "gate liberado por decisao humana em {ts} — pedido {}",
            &c.proposta.gate_sha256[..16]
        ));
    }
}

// --- Montagem, com I/O --------------------------------------------------------

/// Tudo o que as duas fases calculam a partir do disco. Uma rota so: fases que
/// montassem a proposta por caminhos diferentes nao poderiam ser comparadas.
struct Composicao {
    mapeamento: Mapeamento,
    laudo: Laudo,
    catalogo: Catalogo,
    proposta: Proposta,
    /// O contrato como ficaria. Ainda nao escrito em `contracts/`.
    yaml_enriquecido: String,
    /// Campos classificados sem propriedade correspondente no YAML.
    nao_aplicados: Vec<String>,
}

fn compor(run: &mut Run, fase: &str) -> Result<Composicao, String> {
    let c = f3_classificar::laudo_atual(run, "f4", fase)?;
    let (laudo, catalogo, mapeamento) = (c.laudo, c.catalogo, c.mapeamento);

    let alvo = run.contrato.clone();
    let path = run.cfg.root.join(&alvo);
    let bruto = fs::read_to_string(&path)
        .map_err(|e| format!("contrato `{alvo}` ilegivel em {} ({e})", path.display()))?;

    let declarados = declaracao_do_yaml(&bruto).map_err(|e| format!("{e:#}"))?;
    let campos = confrontar(&laudo.campos, &declarados);
    let gate = itens_de_gate(&campos);
    let (yaml_enriquecido, nao_aplicados) =
        aplicar(&bruto, &laudo.campos).map_err(|e| format!("{e:#}"))?;

    let proposta = Proposta {
        contrato: alvo,
        contrato_sha256: laudo.contrato_sha256.clone(),
        glossario_versao: laudo.glossario_versao.clone(),
        catalogo_versao: laudo.catalogo_versao.clone(),
        resumo: resumir(&campos),
        gate_sha256: hash_do_gate(&gate),
        gate,
        campos,
    };

    Ok(Composicao {
        mapeamento,
        laudo,
        catalogo,
        proposta,
        yaml_enriquecido,
        nao_aplicados,
    })
}

fn gravar_proposta(run: &mut Run, c: &Composicao) -> Result<(), String> {
    let json = format!("evidence/{}/f4-proposta.json", run.tracer.run_id());
    let serializado =
        serializar(&c.proposta).map_err(|e| format!("serializando a proposta: {e:#}"))?;
    fs::write(run.cfg.root.join(&json), &serializado)
        .map_err(|e| format!("escrevendo {json}: {e}"))?;

    let yaml = caminho_do_enriquecido(run);
    fs::write(run.cfg.root.join(&yaml), &c.yaml_enriquecido)
        .map_err(|e| format!("escrevendo {yaml}: {e}"))?;

    run.note(format!("proposta {json} | contrato proposto {yaml}"));
    Ok(())
}

fn caminho_do_enriquecido(run: &Run) -> String {
    format!(
        "evidence/{}/f4-contrato-enriquecido.odcs.yaml",
        run.tracer.run_id()
    )
}

/// A comparacao byte a byte com o que `implement` propos. So existe quando as
/// duas fases rodaram no mesmo run — rodando `verify` sozinho nao ha com o que
/// comparar, e a nota diz isso em vez de omitir.
fn conferir_contra_implement(run: &mut Run, c: &Composicao) -> Result<bool, String> {
    let json = run.evidence_dir.join("f4-proposta.json");
    let yaml = run.evidence_dir.join("f4-contrato-enriquecido.odcs.yaml");
    if !json.exists() || !yaml.exists() {
        run.note("sem proposta de `implement` neste run — nada a conferir".to_string());
        return Ok(false);
    }

    let recomputado =
        serializar(&c.proposta).map_err(|e| format!("serializando a proposta: {e:#}"))?;
    let gravado_json =
        fs::read_to_string(&json).map_err(|e| format!("lendo a proposta de `implement`: {e}"))?;
    let gravado_yaml = fs::read_to_string(&yaml)
        .map_err(|e| format!("lendo o contrato proposto por `implement`: {e}"))?;

    if gravado_json != recomputado || gravado_yaml != c.yaml_enriquecido {
        return Err(
            "recomputacao difere da proposta de `implement` — alguma entrada mudou no meio do run"
                .to_string(),
        );
    }

    run.note("recomputacao bate com a proposta de `implement`".to_string());
    Ok(true)
}

/// Roda o lint sobre o contrato **enriquecido**, e nao sobre a fonte.
///
/// F1 ja garante que o contrato entra valido; este e o unico lugar onde ainda
/// dava para quebrar o padrao — escrevendo nele. Fecha o criterio do
/// `contexto.md` sobre a saida.
fn lint_do_enriquecido(run: &mut Run) -> Result<Vec<String>, String> {
    let proposta = caminho_do_enriquecido(run);
    let destino = format!("evidence/{}/f4-lint-enriquecido.json", run.tracer.run_id());

    let saida = run
        .datacontract(
            "verify-lint-enriquecido",
            &[
                "lint",
                &proposta,
                "--output-format",
                "json",
                "--output",
                &destino,
            ],
        )
        .map_err(|e| format!("{e}"))?;

    let bruto = fs::read_to_string(run.cfg.root.join(&destino)).map_err(|e| {
        format!(
            "lint do enriquecido saiu com {} e nao deixou relatorio em {destino} ({e})",
            saida.exit_code
        )
    })?;
    let veredito = f1_validar::ler_veredito(&bruto).map_err(|e| format!("{e:#}"))?;

    run.note(format!(
        "lint do contrato enriquecido {} — {} check(s), relatorio {destino}",
        if veredito.passed { "PASS" } else { "FAIL" },
        veredito.checks
    ));

    if veredito.passed {
        return Ok(Vec::new());
    }
    if veredito.failures.is_empty() {
        return Ok(vec![format!(
            "lint do enriquecido reprovou sem check reprovado no relatorio (exit {})",
            saida.exit_code
        )]);
    }
    Ok(veredito
        .failures
        .iter()
        .map(|f| format!("contrato enriquecido invalido — {f}"))
        .collect())
}

/// O ultimo ato de `verify`: escrever o enriquecimento onde o contrato mora.
///
/// Depois do veredito, nunca antes. Um `implement` que escrevesse aqui deixaria
/// o repositorio num estado que nenhuma fase aprovou, e um `Blocked` teria de
/// saber desfazer o que escreveu.
fn aplicar_no_contrato(run: &mut Run, c: &Composicao) -> Outcome {
    // Do que a proposta diz, e nao de `run`: e o mesmo caminho, e ler daqui faz
    // o artefato julgado e o arquivo escrito serem o mesmo por construcao.
    let alvo = c.proposta.contrato.clone();
    let destino = run.cfg.root.join(&alvo);
    let atual = fs::read_to_string(&destino).unwrap_or_default();

    if atual == c.yaml_enriquecido {
        run.note(format!(
            "contrato {alvo} ja reflete a classificacao vigente — nada a escrever"
        ));
        return Outcome::Pass;
    }

    if let Err(e) = fs::write(&destino, &c.yaml_enriquecido) {
        return Outcome::Fail(format!("escrevendo {alvo}: {e}"));
    }
    run.note(format!(
        "contrato {alvo} enriquecido — {} campo(s) marcado(s); o diff vai no commit do handoff",
        c.proposta.resumo.classificados
    ));
    Outcome::Pass
}

// --- O laudo ---------------------------------------------------------------------
//
// "Laudo" tem dois sentidos neste codigo, e vale separa-los: `f3::Laudo` e a
// classificacao como estrutura de dados; o laudo desta secao e o **documento**
// entregue — o que o time de governanca hoje escreve a mao.
//
// O documento nao e evidencia de execucao, e por isso nao mora em
// `evidence/<run_id>/`: evidencia e regeneravel e descartavel, laudo e registro
// emitido. Ele fica ao lado do contrato, versionado, e entra no mesmo commit
// que a classificacao a que se refere.

/// Onde o laudo de um contrato e arquivado.
///
/// Nome por **versao do contrato + sha256 do contrato classificado**: a versao
/// para um humano achar, o sha para nao sobrescrever. Dois laudos da mesma
/// versao existem de verdade — mesma `version` reclassificada por um catalogo
/// novo sao duas constatacoes diferentes, e apagar a primeira destruiria
/// justamente o que a auditoria quer comparar.
fn caminho_do_laudo(contrato: &str, versao: &str, sha: &str) -> String {
    // Ao lado do contrato, seja qual for a profundidade dele: `contracts/x/` ou
    // `contracts/<dominio>/<contrato>/` produzem `laudos/` no mesmo nivel do
    // `.yaml`, sem o harness precisar saber qual layout a organizacao adotou.
    let dir = std::path::Path::new(contrato)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| contrato::DIRETORIO.to_string());
    format!("{dir}/laudos/{versao}-{}.md", &sha[..7])
}

/// Funcao pura: a `version` que o contrato declara.
///
/// Erro e nao default: a versao entra no nome do arquivo, e um contrato sem ela
/// produziria `-a1b2c3d.md` — legivel para ninguem, e dois contratos sem versao
/// nao colidiriam so por sorte do sha.
pub fn versao_do_yaml(bruto: &str) -> Result<String> {
    let doc: Value = serde_norway::from_str(bruto).context("contrato nao e YAML valido")?;
    // `version: 1.0` e numero em YAML, nao string. Recusar por tipo faria o
    // harness reprovar um contrato que o ODCS aceita.
    let versao = match doc.get("version") {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => bail!("contrato sem `version` — o laudo precisa dela para ser identificavel"),
    };
    if versao.is_empty() {
        bail!("contrato com `version` vazia — o laudo precisa dela para ser identificavel");
    }
    if versao.contains(['/', '\\', ':']) || versao.contains(char::is_whitespace) {
        bail!("`version` `{versao}` nao serve como nome de arquivo para o laudo");
    }
    Ok(versao)
}

/// Emite o laudo, ou reconhece que ele ja foi emitido para este conteudo.
fn gravar_laudo(run: &mut Run, c: &Composicao) -> Result<(), String> {
    // A versao e o sha saem do contrato **classificado**, e nao da fonte: e o
    // arquivo classificado que fica no repositorio, e e a ele que o laudo
    // responde.
    let versao = versao_do_yaml(&c.yaml_enriquecido).map_err(|e| format!("{e:#}"))?;
    let sha = tools::sha256_hex(&c.yaml_enriquecido);
    let destino = caminho_do_laudo(&c.proposta.contrato, &versao, &sha);
    let corpo = documento_do_laudo(&c.proposta, &c.laudo, &versao, &sha);

    let path = run.cfg.root.join(&destino);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("criando {}: {e}", dir.display()))?;
    }

    // Reemitir identico nao e escrita: o documento e deterministico, entao o
    // mesmo conteudo no mesmo caminho significa que este laudo ja existe. Sem
    // isto, rodar o fluxo duas vezes sujaria o `git status` sem nada ter mudado.
    if fs::read_to_string(&path).is_ok_and(|atual| atual == corpo) {
        run.note(format!(
            "laudo {destino} ja emitido para este conteudo — nada a escrever"
        ));
        return Ok(());
    }

    fs::write(&path, &corpo).map_err(|e| format!("escrevendo {destino}: {e}"))?;
    run.note(format!("laudo emitido em {destino}"));
    Ok(())
}

/// Um sha256 encurtado para leitura, sem poder entrar em panico.
///
/// `&sha[..16]` e o idioma do resto do arquivo e serve porque ali os hashes vem
/// sempre de `sha256_hex`. Aqui nao vale correr o risco: o laudo e o unico
/// artefato que sai deste repositorio para quem nao roda o harness, e um panico
/// na formatacao de um cabecalho derrubaria a fase inteira.
fn curto(sha: &str) -> &str {
    &sha[..sha.len().min(16)]
}

/// Uma celula de tabela markdown.
///
/// A justificativa vem do catalogo, escrita por gente, com quebra de linha do
/// YAML e eventualmente um `|` no meio. Sem achatar e escapar, uma entrada bem
/// escrita quebraria a tabela inteira do laudo — em silencio.
fn celula(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

/// O laudo, na forma em que e entregue.
///
/// **Sem data e sem `run_id`**, pela mesma razao que a proposta e a
/// classificacao de F3: o mesmo contrato, com o mesmo glossario e o mesmo
/// catalogo, produz este documento byte a byte. Reemitir nunca gera diff, e a
/// data de emissao e a do commit — o Git ja e a autoridade de tempo aqui, e um
/// carimbo no corpo faria o arquivo mudar sem que a analise tivesse mudado.
///
/// Pela mesma razao ele **nao registra a aprovacao**: o documento e a
/// constatacao tecnica. Quem assina e o merge, e a revisao que o autorizou fica
/// no historico, presa ao mesmo sha256 que esta aqui no cabecalho.
fn documento_do_laudo(p: &Proposta, l: &Laudo, versao: &str, sha_final: &str) -> String {
    let mut s = String::new();

    s.push_str("# Laudo de classificacao de privacidade\n\n");
    s.push_str(&format!(
        "**Contrato** `{}` v{versao}  \n**sha256** `{sha_final}`\n\n",
        p.contrato
    ));
    s.push_str(
        "O sha256 acima e o do contrato **classificado** — o arquivo que esta neste \
         repositorio, ao lado deste laudo. Quem auditar confere a correspondencia com \
         qualquer ferramenta de hash, sem depender deste projeto.\n\n",
    );

    s.push_str("## Criterio aplicado\n\n");
    s.push_str("| Insumo | Versao | sha256 |\n|---|---|---|\n");
    s.push_str(&format!(
        "| Glossario `{}` | {} | `{}` |\n",
        l.glossario,
        l.glossario_versao,
        curto(&l.glossario_sha256)
    ));
    s.push_str(&format!(
        "| Catalogo `{}` | {} | `{}` |\n\n",
        l.catalogo,
        l.catalogo_versao,
        curto(&l.catalogo_sha256)
    ));
    s.push_str(
        "A classificacao e **consulta a catalogo**: cada campo do contrato e mapeado a um \
         termo do glossario, e o termo carrega a classificacao, a justificativa e a base \
         legal. Nao ha inferencia sobre nome de campo e nenhum modelo participa da decisao \
         — dois campos que casam com o mesmo termo recebem a mesma resposta em qualquer \
         contrato, em qualquer data.\n\n",
    );

    s.push_str("## Resumo\n\n");
    s.push_str(&format!("- {} campo(s) analisado(s)\n", p.resumo.campos));
    s.push_str(&format!(
        "- {} classificado(s) — {} PII, {} sensivel\n",
        p.resumo.classificados, l.resumo.pii, l.resumo.sensivel
    ));
    s.push_str(&format!(
        "- por nivel: {}\n",
        niveis_legivel(&l.resumo.por_nivel)
    ));
    s.push_str(&format!(
        "- {} sem classificacao\n\n",
        p.resumo.lacunas + p.resumo.conflitos
    ));

    s.push_str("## Classificacao por campo\n\n");
    s.push_str(
        "| Campo | Termo | `classification` | PII | Sensivel | Base legal | Justificativa |\n",
    );
    s.push_str("|---|---|---|---|---|---|---|\n");
    for c in &p.campos {
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            c.campo,
            c.termo
                .as_deref()
                .map(|t| format!("`{t}`"))
                .unwrap_or_else(|| "—".to_string()),
            match &c.proposto.classification {
                Some(n) => format!("`{n}`"),
                None => "**sem classificacao**".to_string(),
            },
            sim_nao(c.proposto.pii),
            sim_nao(c.proposto.sensivel),
            celula(c.referencia.as_deref().unwrap_or("—")),
            celula(&c.justificativa),
        ));
    }

    s.push_str("\n## Pendencias de decisao humana\n\n");
    if p.gate.is_empty() {
        s.push_str(
            "Nenhuma. Todo campo classificado veio do catalogo sem contrariar decisao \
             anterior, e nenhum campo ficou sem decisao.\n",
        );
    } else {
        s.push_str(&format!(
            "{} item(ns), sob o pedido `{}`. Enquanto houver pendencia aqui, o campo \
             correspondente **nao** recebe classificacao no contrato: o harness nao escreve \
             \"nao sei\" num contrato de dados.\n\n",
            p.gate.len(),
            curto(&p.gate_sha256)
        ));
        s.push_str("| Tipo | Campo | Detalhe |\n|---|---|---|\n");
        for i in &p.gate {
            s.push_str(&format!(
                "| `{}` | `{}` | {} |\n",
                i.tipo.rotulo(),
                i.campo,
                celula(&i.detalhe)
            ));
        }
        s.push_str(
            "\nLacuna se resolve ampliando o glossario e o catalogo — ou aceitando, por \
             decisao registrada, que o campo siga sem classificacao.\n",
        );
    }

    s.push_str("\n## Sobre este documento\n\n");
    s.push_str(
        "E deterministico: o mesmo contrato, com o mesmo glossario e o mesmo catalogo, \
         produz este arquivo byte a byte. Nao ha data no corpo de proposito — a data de \
         emissao e a do commit, e o Git ja responde por ela.\n\n\
         Nao ha aprovacao no corpo pelo mesmo motivo: este e o laudo tecnico, e quem \
         assina e o merge. A revisao que o autorizou fica no historico do repositorio, \
         presa ao mesmo sha256 do cabecalho.\n\n\
         Subir a versao do catalogo pode mudar a classificacao de um termo sem que o \
         contrato mude. Quando isso acontecer, este laudo continua valendo para o criterio \
         que o gerou, e um novo sera emitido ao lado dele.\n",
    );

    s
}

/// Grava o pedido de gate e para o fluxo.
///
/// O pedido mora em `state/`, e nao em `evidence/`: e estado do ciclo, e quem
/// o consome e o comando `approve`. A evidencia do run guarda a proposta
/// inteira; o pedido guarda o que precisa de resposta.
fn bloquear(run: &mut Run, p: &Proposta) -> Outcome {
    let itens: Vec<String> = p.gate.iter().map(ItemDeGate::linha).collect();
    let resumo = format!(
        "{} lacuna(s), {} reclassificacao(oes), {} conflito(s)",
        p.resumo.lacunas, p.resumo.reclassificacoes, p.resumo.conflitos
    );

    let pedido = GatePendente {
        schema_version: SCHEMA_VERSION,
        feature: FEATURE.to_string(),
        gate_sha256: p.gate_sha256.clone(),
        run_id: run.tracer.run_id().to_string(),
        criado_em: trace::now_rfc3339(),
        resumo: resumo.clone(),
        itens: itens.clone(),
    };
    if let Err(e) = pedido.save(&run.cfg.gate_pendente_path()) {
        return Outcome::Fail(format!("gravando o pedido de gate: {e:#}"));
    }

    for linha in &itens {
        run.note(format!("  gate — {linha}"));
    }
    run.note(format!(
        "pedido {} registrado em state/gate-pendente.json — libere com `./run.sh approve {FEATURE}`",
        &p.gate_sha256[..16]
    ));

    Outcome::Blocked(format!("{resumo} aguardando decisao humana"))
}

fn gravar_veredito(run: &mut Run, v: &Veredito) -> Result<()> {
    let destino = format!("evidence/{}/f4-veredito.json", run.tracer.run_id());
    let corpo = serde_json::to_string_pretty(v).context("serializando o veredito")?;
    fs::write(run.cfg.root.join(&destino), corpo)
        .with_context(|| format!("escrevendo {destino}"))?;
    run.note(format!("veredito {destino}"));
    Ok(())
}

fn relatorio_legivel(
    run: &mut Run,
    c: &Composicao,
    aprovado_em: Option<&str>,
) -> Result<(), String> {
    let destino = format!("evidence/{}/f4-relatorio.md", run.tracer.run_id());
    fs::write(
        run.cfg.root.join(&destino),
        markdown(&c.proposta, aprovado_em),
    )
    .map_err(|e| format!("escrevendo {destino}: {e}"))?;
    run.note(format!("relatorio {destino}"));
    Ok(())
}

// --- O que o contrato declara ---------------------------------------------------

/// A marcacao de privacidade de um campo, na forma em que o ODCS a guarda.
///
/// Serve para os dois lados da comparacao: o que o catalogo propoe e o que o
/// contrato ja declara. Um tipo so, porque compara-los e literalmente `==` — e
/// e nesse `==` que o gate mora.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Marcacao {
    pub classification: Option<String>,
    /// `tags: [pii]`. O ODCS 3.1.0 nao tem campo `pii`.
    pub pii: Option<bool>,
    /// `tags: [sensitive]`.
    pub sensivel: Option<bool>,
}

impl Marcacao {
    /// Nada declarado. Diferente de "declarado como nao-PII": ausencia nao e
    /// decisao, e e essa distincao que separa primeira classificacao de
    /// reclassificacao.
    pub fn vazia(&self) -> bool {
        self.classification.is_none() && self.pii.is_none() && self.sensivel.is_none()
    }

    /// `vazio` e a palavra para a ausencia, e ela muda com o lado da
    /// comparacao: o contrato **nao declara**, o catalogo **nao classifica**.
    /// Uma frase so para os dois lados leria errado numa das colunas.
    fn legivel(&self, vazio: &str) -> String {
        match &self.classification {
            None if self.vazia() => vazio.to_string(),
            c => format!(
                "{}{}{}",
                c.clone()
                    .unwrap_or_else(|| "sem classification".to_string()),
                match self.pii {
                    Some(true) => ", pii",
                    Some(false) => ", nao pii",
                    None => "",
                },
                match self.sensivel {
                    Some(true) => ", sensivel",
                    Some(false) => "",
                    None => "",
                }
            ),
        }
    }
}

/// Funcao pura: o que cada propriedade do contrato declara hoje.
///
/// A regra de leitura da tag: propriedade que declara `classification` ja
/// passou por classificacao, entao a ausencia da tag `pii` ali significa "nao
/// e PII" — decisao. Propriedade que nao declara nada nao decidiu nada.
pub fn declaracao_do_yaml(bruto: &str) -> Result<BTreeMap<String, Marcacao>> {
    let doc: Value = serde_norway::from_str(bruto).context("contrato nao e YAML valido")?;
    let mut out = BTreeMap::new();

    let Some(schemas) = doc.get("schema").and_then(Value::as_sequence) else {
        return Ok(out);
    };
    for objeto in schemas {
        let Some(props) = objeto.get("properties").and_then(Value::as_sequence) else {
            continue;
        };
        for prop in props {
            let Some(nome) = prop.get("name").and_then(Value::as_str) else {
                continue;
            };
            let classification = prop
                .get("classification")
                .and_then(Value::as_str)
                .map(str::to_string);
            let tags = tags_de(prop);
            let ja_classificada =
                classification.is_some() || tags.iter().any(|t| t == "pii" || t == "sensitive");

            out.insert(
                nome.to_string(),
                Marcacao {
                    classification,
                    pii: ja_classificada.then(|| tags.iter().any(|t| t == "pii")),
                    sensivel: ja_classificada.then(|| tags.iter().any(|t| t == "sensitive")),
                },
            );
        }
    }
    Ok(out)
}

fn tags_de(prop: &Value) -> Vec<String> {
    prop.get("tags")
        .and_then(Value::as_sequence)
        .map(|s| {
            s.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

// --- O confronto ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mudanca {
    /// O contrato nao declarava nada. Nao abre gate.
    PrimeiraClassificacao,
    /// O contrato ja declarava exatamente isto. Nao abre gate.
    Inalterado,
    /// O contrato declara outra coisa. **Gate.**
    Reclassificacao,
    /// Sem classificacao a propor, e nada declarado. **Gate.**
    Lacuna,
    /// Sem classificacao a propor, mas o contrato declara algo. **Gate.**
    Conflito,
}

impl Mudanca {
    fn tipo_de_gate(self) -> Option<TipoDeGate> {
        match self {
            Mudanca::PrimeiraClassificacao | Mudanca::Inalterado => None,
            Mudanca::Reclassificacao => Some(TipoDeGate::Reclassificacao),
            Mudanca::Lacuna => Some(TipoDeGate::Lacuna),
            Mudanca::Conflito => Some(TipoDeGate::Conflito),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoDeGate {
    Lacuna,
    Reclassificacao,
    Conflito,
}

impl TipoDeGate {
    fn rotulo(self) -> &'static str {
        match self {
            TipoDeGate::Lacuna => "lacuna",
            TipoDeGate::Reclassificacao => "reclassificacao",
            TipoDeGate::Conflito => "conflito",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemDeGate {
    pub tipo: TipoDeGate,
    pub campo: String,
    pub detalhe: String,
}

impl ItemDeGate {
    /// A linha que o humano le no pedido. Sem hash, sem run_id: quem aprova
    /// decide sobre o conteudo.
    pub fn linha(&self) -> String {
        format!("[{}] {} — {}", self.tipo.rotulo(), self.campo, self.detalhe)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CampoDoGate {
    pub campo: String,
    pub termo: Option<String>,
    pub mudanca: Mudanca,
    pub proposto: Marcacao,
    pub declarado: Marcacao,
    /// Por que o catalogo classifica assim. Vem da entrada do catalogo, via F3:
    /// o harness carrega a justificativa, nunca a redige.
    ///
    /// Sem isto o laudo sai com a conclusao e sem o criterio — que e a
    /// diferenca entre um laudo e uma tabela.
    pub justificativa: String,
    /// A base legal da decisao, como o catalogo a declara.
    ///
    /// `None` quando nao ha classificacao a propor: lacuna nao tem artigo de
    /// lei, tem ausencia de vocabulario.
    pub referencia: Option<String>,
}

/// Funcao pura: proposta do catalogo x o que o contrato declara.
pub fn confrontar(
    campos: &[CampoClassificado],
    declarados: &BTreeMap<String, Marcacao>,
) -> Vec<CampoDoGate> {
    campos
        .iter()
        .map(|c| {
            let proposto = Marcacao {
                classification: c.classification.map(|n| n.rotulo().to_string()),
                pii: c.pii,
                sensivel: c.sensivel,
            };
            let declarado = declarados.get(&c.campo).cloned().unwrap_or_default();

            let mudanca = match c.situacao {
                Situacao::Classificado if declarado.vazia() => Mudanca::PrimeiraClassificacao,
                Situacao::Classificado if proposto == declarado => Mudanca::Inalterado,
                Situacao::Classificado => Mudanca::Reclassificacao,
                Situacao::NaoClassificado if declarado.vazia() => Mudanca::Lacuna,
                Situacao::NaoClassificado => Mudanca::Conflito,
            };

            CampoDoGate {
                campo: c.campo.clone(),
                termo: c.termo.clone(),
                mudanca,
                proposto,
                declarado,
                // Passam por aqui sem serem tocadas. F3 ja as leu do catalogo,
                // e reescrever qualquer uma delas seria o harness opinando
                // sobre a decisao em vez de transporta-la.
                justificativa: c.justificativa.clone(),
                referencia: c.referencia.clone(),
            }
        })
        .collect()
}

/// Funcao pura: o que exige decisao humana. Lista vazia = o fluxo segue.
pub fn itens_de_gate(campos: &[CampoDoGate]) -> Vec<ItemDeGate> {
    campos
        .iter()
        .filter_map(|c| {
            let tipo = c.mudanca.tipo_de_gate()?;
            let detalhe = match tipo {
                TipoDeGate::Lacuna => "campo sem termo no glossario — segue sem classificacao \
                     no contrato ate decisao humana"
                    .to_string(),
                TipoDeGate::Reclassificacao => format!(
                    "o contrato declara `{}` e o catalogo diz `{}`",
                    c.declarado.legivel(NADA_DECLARADO),
                    c.proposto.legivel(SEM_CLASSIFICACAO)
                ),
                TipoDeGate::Conflito => format!(
                    "o contrato declara `{}` e o catalogo nao sabe classificar este campo",
                    c.declarado.legivel(NADA_DECLARADO)
                ),
            };
            Some(ItemDeGate {
                tipo,
                campo: c.campo.clone(),
                detalhe,
            })
        })
        .collect()
}

/// Funcao pura: a identidade do pedido de gate.
///
/// **Do conteudo, nao da feature.** Aprovar carimba um conjunto de itens; mudou
/// o contrato, o glossario ou o catalogo, os itens mudam, o hash muda e o gate
/// fecha de novo. Uma aprovacao carimbada na feature seria um passe permanente.
pub fn hash_do_gate(itens: &[ItemDeGate]) -> String {
    let corpo = serde_json::to_string_pretty(itens).unwrap_or_default();
    tools::sha256_hex(&corpo)
}

// --- O enriquecimento -----------------------------------------------------------

/// Funcao pura: contrato + classificacao -> contrato enriquecido, mais os
/// campos classificados que nao acharam propriedade correspondente.
///
/// Idempotente: aplicar duas vezes produz o mesmo arquivo. E o que permite
/// rodar F4 de novo sem gerar diff, e o que faz o gate ficar quieto quando nada
/// mudou.
///
/// Campo sem classificacao **nao e tocado**: o harness nao escreve "nao sei" no
/// contrato.
///
/// Custo assumido: o YAML e reserializado inteiro, entao comentarios e linhas
/// em branco se perdem. A alternativa — costurar linhas por posicao para
/// preservar formatacao — quebra no primeiro contrato com indentacao diferente.
pub fn aplicar(bruto: &str, campos: &[CampoClassificado]) -> Result<(String, Vec<String>)> {
    let mut doc: Value = serde_norway::from_str(bruto).context("contrato nao e YAML valido")?;

    let por_nome: BTreeMap<&str, &CampoClassificado> = campos
        .iter()
        .filter(|c| c.situacao == Situacao::Classificado)
        .map(|c| (c.campo.as_str(), c))
        .collect();
    let mut aplicados: BTreeSet<String> = BTreeSet::new();

    if let Some(schemas) = doc.get_mut("schema").and_then(Value::as_sequence_mut) {
        for objeto in schemas.iter_mut() {
            let Some(props) = objeto
                .get_mut("properties")
                .and_then(Value::as_sequence_mut)
            else {
                continue;
            };
            for prop in props.iter_mut() {
                let Some(nome) = prop.get("name").and_then(Value::as_str).map(str::to_string)
                else {
                    continue;
                };
                let Some(c) = por_nome.get(nome.as_str()) else {
                    continue;
                };
                marcar(prop, c);
                aplicados.insert(nome);
            }
        }
    }

    let yaml = serde_norway::to_string(&doc).context("serializando o contrato enriquecido")?;
    let nao_aplicados = por_nome
        .keys()
        .filter(|n| !aplicados.contains(**n))
        .map(|n| n.to_string())
        .collect();

    Ok((yaml, nao_aplicados))
}

/// Escreve os tres campos ODCS numa propriedade. Tudo deterministico: a
/// comparacao byte a byte entre `implement` e `verify` depende disso.
fn marcar(prop: &mut Value, c: &CampoClassificado) {
    let Some(nivel) = c.classification else {
        return;
    };
    let Some(m) = prop.as_mapping_mut() else {
        return;
    };

    m.insert(
        Value::from("classification"),
        Value::from(nivel.rotulo().to_string()),
    );

    // Tag alheia e preservada: `finance` ou `employee_record` nao sao nossas
    // para remover. So as duas que este catalogo governa sao reescritas.
    let mut tags: Vec<String> = m
        .get("tags")
        .and_then(Value::as_sequence)
        .map(|s| {
            s.iter()
                .filter_map(Value::as_str)
                .filter(|t| *t != "pii" && *t != "sensitive")
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if c.pii == Some(true) {
        tags.push("pii".to_string());
    }
    if c.sensivel == Some(true) {
        tags.push("sensitive".to_string());
    }
    if tags.is_empty() {
        m.shift_remove("tags");
    } else {
        m.insert(
            Value::from("tags"),
            Value::Sequence(tags.into_iter().map(Value::from).collect()),
        );
    }

    // Vinculo com o glossario. Entrada alheia e preservada pelo mesmo motivo
    // das tags — so a que aponta para o nosso glossario e reescrita.
    if let Some(termo) = c.termo.as_deref() {
        let url = format!("{URL_GLOSSARIO}#{termo}");
        let mut defs: Vec<Value> = m
            .get("authoritativeDefinitions")
            .and_then(Value::as_sequence)
            .map(|s| {
                s.iter()
                    .filter(|d| {
                        !d.get("url")
                            .and_then(Value::as_str)
                            .is_some_and(|u| u.starts_with(&format!("{URL_GLOSSARIO}#")))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        let mut nova = serde_norway::Mapping::new();
        nova.insert(Value::from("url"), Value::from(url));
        nova.insert(Value::from("type"), Value::from(TIPO_DEFINICAO));
        defs.push(Value::Mapping(nova));

        m.insert(
            Value::from("authoritativeDefinitions"),
            Value::Sequence(defs),
        );
    }
}

// --- Proposta, veredito e relatorio ----------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResumoGate {
    pub campos: usize,
    pub classificados: usize,
    pub primeira_classificacao: usize,
    pub inalterados: usize,
    pub lacunas: usize,
    pub reclassificacoes: usize,
    pub conflitos: usize,
}

/// A proposta de enriquecimento.
///
/// **Sem `run_id` e sem timestamp**, como em F2 e F3: o mesmo contrato com o
/// mesmo glossario e o mesmo catalogo produz o mesmo arquivo em qualquer run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Proposta {
    pub contrato: String,
    pub contrato_sha256: String,
    pub glossario_versao: String,
    pub catalogo_versao: String,
    pub resumo: ResumoGate,
    pub gate_sha256: String,
    pub gate: Vec<ItemDeGate>,
    pub campos: Vec<CampoDoGate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Veredito {
    pub aprovado: bool,
    pub conferido_contra_implement: bool,
    pub glossario_versao: String,
    pub catalogo_versao: String,
    pub resumo: ResumoGate,
    pub gate_sha256: String,
    pub gate: Vec<ItemDeGate>,
    /// Quando o gate foi liberado, se foi.
    pub aprovado_em: Option<String>,
    pub defeitos: Vec<String>,
}

fn resumir(campos: &[CampoDoGate]) -> ResumoGate {
    let conta = |m: Mudanca| campos.iter().filter(|c| c.mudanca == m).count();
    let primeira = conta(Mudanca::PrimeiraClassificacao);
    let inalterados = conta(Mudanca::Inalterado);
    let reclassificacoes = conta(Mudanca::Reclassificacao);

    ResumoGate {
        campos: campos.len(),
        classificados: primeira + inalterados + reclassificacoes,
        primeira_classificacao: primeira,
        inalterados,
        lacunas: conta(Mudanca::Lacuna),
        reclassificacoes,
        conflitos: conta(Mudanca::Conflito),
    }
}

/// Serializacao unica: `implement` grava com ela e `verify` recomputa com ela.
fn serializar(p: &Proposta) -> Result<String> {
    serde_json::to_string_pretty(p).context("serializando a proposta")
}

fn markdown(p: &Proposta, aprovado_em: Option<&str>) -> String {
    let mut s = String::new();
    s.push_str("# F4 — Enriquecimento, lacunas e gate\n\n");
    s.push_str(&format!(
        "- Contrato: `{}` (sha256 `{}` na entrada)\n",
        p.contrato,
        &p.contrato_sha256[..16]
    ));
    s.push_str(&format!(
        "- Glossario v{} · catalogo v{}\n",
        p.glossario_versao, p.catalogo_versao
    ));
    s.push_str(&format!(
        "- {} campo(s) — {} classificado(s): {} primeira classificacao, {} inalterado(s), \
         {} reclassificacao(oes)\n",
        p.resumo.campos,
        p.resumo.classificados,
        p.resumo.primeira_classificacao,
        p.resumo.inalterados,
        p.resumo.reclassificacoes
    ));
    s.push_str(&format!(
        "- {} lacuna(s), {} conflito(s)\n\n",
        p.resumo.lacunas, p.resumo.conflitos
    ));

    s.push_str("## Pendencias de decisao humana\n\n");
    if p.gate.is_empty() {
        s.push_str(
            "Nenhuma. Todo campo classificado veio do catalogo sem contrariar decisao anterior, \
             e nenhum campo ficou sem decisao.\n\n",
        );
    } else {
        match aprovado_em {
            Some(ts) => s.push_str(&format!(
                "Submetidas ao gate e **liberadas por decisao humana em {ts}** — pedido \
                 `{}`.\n\n",
                &p.gate_sha256[..16]
            )),
            None => s.push_str(&format!(
                "**Pendentes.** Pedido `{}` — libere com `./run.sh approve {FEATURE}`.\n\n",
                &p.gate_sha256[..16]
            )),
        }
        s.push_str("| Tipo | Campo | Detalhe |\n|---|---|---|\n");
        for i in &p.gate {
            s.push_str(&format!(
                "| `{}` | `{}` | {} |\n",
                i.tipo.rotulo(),
                i.campo,
                i.detalhe
            ));
        }
        s.push('\n');
    }

    s.push_str("## Classificacao aplicada\n\n");
    s.push_str("| Campo | Termo | Mudanca | Proposto | Declarado antes |\n");
    s.push_str("|---|---|---|---|---|\n");
    for c in &p.campos {
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            c.campo,
            c.termo
                .as_deref()
                .map(|t| format!("`{t}`"))
                .unwrap_or_else(|| "—".to_string()),
            match c.mudanca {
                Mudanca::PrimeiraClassificacao => "primeira classificacao",
                Mudanca::Inalterado => "inalterado",
                Mudanca::Reclassificacao => "**reclassificacao**",
                Mudanca::Lacuna => "**lacuna**",
                Mudanca::Conflito => "**conflito**",
            },
            c.proposto.legivel(SEM_CLASSIFICACAO),
            c.declarado.legivel(NADA_DECLARADO)
        ));
    }

    s.push_str(
        "\nCampo sem classificacao nao e tocado no contrato: o harness nao escreve \"nao sei\" \
         num contrato de dados. Resolver a lacuna e ampliar o glossario e o catalogo — ou \
         aceitar, por decisao registrada, que o campo siga sem classificacao.\n\n\
         O que foi escrito em cada propriedade classificada e ODCS 3.1.0: `classification`, \
         `tags: [pii, sensitive]` e `authoritativeDefinitions` apontando para o termo do \
         glossario.\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O contrato dos testes. Antes era `contrato::CAMINHO`; agora que o alvo e
    /// resolvido por run, o valor entra por parametro tambem aqui.
    const ALVO: &str = "contracts/clientes/contract.odcs.yaml";
    use crate::features::contrato::Campo;
    use crate::features::f2_mapear::{carregar_glossario, mapear};
    use crate::features::f3_classificar::{carregar_catalogo, classificar};

    const CONTRATO: &str = r#"
apiVersion: v3.1.0
kind: DataContract
id: teste
version: 1.0.0
status: draft
schema:
  - name: clientes
    properties:
      - name: cpf
        logicalType: string
      - name: data_cadastro
        logicalType: date
      - name: segmento
        logicalType: string
"#;

    fn glossario() -> crate::features::f2_mapear::Glossario {
        carregar_glossario(
            r#"
version: 1.0.0
termos:
  - id: pessoa.cpf
    nome: CPF
    definicao: Numero de inscricao no CPF.
    aliases: [cpf]
  - id: cadastro.data_criacao
    nome: Data de criacao
    definicao: Data em que o registro nasceu.
    aliases: [data_cadastro]
"#,
        )
        .unwrap()
    }

    fn catalogo() -> Catalogo {
        carregar_catalogo(
            r#"
version: 1.0.0
classificacoes:
  - termo: pessoa.cpf
    classification: restricted
    pii: true
    sensivel: false
    justificativa: Identificador univoco de pessoa natural.
    referencia: LGPD art. 5, I
  - termo: cadastro.data_criacao
    classification: internal
    pii: false
    sensivel: false
    justificativa: Descreve o registro, nao a pessoa.
    referencia: LGPD art. 5, I — por exclusao
"#,
        )
        .unwrap()
    }

    /// Os tres campos do contrato de teste, classificados pelo catalogo.
    fn laudo() -> Laudo {
        let campos: Vec<Campo> = ["cpf", "data_cadastro", "segmento"]
            .iter()
            .map(|n| Campo {
                nome: n.to_string(),
                tipo: "string".to_string(),
            })
            .collect();
        let m = mapear(ALVO, &campos, &glossario(), "sha-c", "sha-g");
        classificar(&m, &catalogo(), "sha-cat")
    }

    fn confronto_de(yaml: &str) -> Vec<CampoDoGate> {
        let l = laudo();
        confrontar(&l.campos, &declaracao_do_yaml(yaml).unwrap())
    }

    // --- Leitura do que o contrato declara ---------------------------------

    #[test]
    fn contrato_sem_marcacao_nao_declara_nada() {
        let d = declaracao_do_yaml(CONTRATO).unwrap();
        assert_eq!(d.len(), 3);
        assert!(d["cpf"].vazia());
    }

    /// Ausencia da tag numa propriedade ja classificada e decisao, nao lacuna.
    #[test]
    fn classification_sem_tag_declara_nao_pii() {
        let yaml = CONTRATO.replace(
            "      - name: data_cadastro\n        logicalType: date",
            "      - name: data_cadastro\n        logicalType: date\n        classification: internal",
        );
        let d = declaracao_do_yaml(&yaml).unwrap();
        assert_eq!(d["data_cadastro"].pii, Some(false));
        assert!(!d["data_cadastro"].vazia());
    }

    // --- O que abre o gate --------------------------------------------------

    /// Contrato virgem: classificar pela primeira vez nao e reclassificacao.
    /// Gate que sempre dispara e gate que ninguem le.
    #[test]
    fn primeira_classificacao_nao_abre_gate() {
        let campos = confronto_de(CONTRATO);
        let cpf = campos.iter().find(|c| c.campo == "cpf").unwrap();
        assert_eq!(cpf.mudanca, Mudanca::PrimeiraClassificacao);
        let itens = itens_de_gate(&campos);
        assert!(
            itens.iter().all(|i| i.campo != "cpf"),
            "cpf nao deveria abrir gate: {itens:?}"
        );
    }

    /// O contrato ja enriquecido, com o catalogo inalterado, fica quieto.
    #[test]
    fn contrato_ja_enriquecido_nao_abre_gate_de_reclassificacao() {
        let (enriquecido, _) = aplicar(CONTRATO, &laudo().campos).unwrap();
        let campos = confronto_de(&enriquecido);
        assert_eq!(
            campos.iter().find(|c| c.campo == "cpf").unwrap().mudanca,
            Mudanca::Inalterado
        );
        assert!(itens_de_gate(&campos).iter().all(|i| i.campo != "cpf"));
    }

    /// A situacao que o `contexto.md` nomeia: despromover um campo PII.
    #[test]
    fn despromocao_de_pii_abre_gate() {
        let yaml = CONTRATO.replace(
            "      - name: cpf\n        logicalType: string",
            "      - name: cpf\n        logicalType: string\n        classification: public",
        );
        let campos = confronto_de(&yaml);
        assert_eq!(
            campos.iter().find(|c| c.campo == "cpf").unwrap().mudanca,
            Mudanca::Reclassificacao
        );
        let item = itens_de_gate(&campos)
            .into_iter()
            .find(|i| i.campo == "cpf")
            .expect("cpf deveria abrir gate");
        assert_eq!(item.tipo, TipoDeGate::Reclassificacao);
        assert!(item.detalhe.contains("public") && item.detalhe.contains("restricted"));
    }

    /// Subir a protecao tambem e sobrescrever decisao humana anterior.
    #[test]
    fn promocao_tambem_abre_gate() {
        let yaml = CONTRATO.replace(
            "      - name: data_cadastro\n        logicalType: date",
            "      - name: data_cadastro\n        logicalType: date\n        classification: confidential\n        tags:\n          - pii",
        );
        let campos = confronto_de(&yaml);
        assert_eq!(
            campos
                .iter()
                .find(|c| c.campo == "data_cadastro")
                .unwrap()
                .mudanca,
            Mudanca::Reclassificacao
        );
    }

    /// A trava do contexto.md: campo sem decisao nao atravessa o fluxo calado.
    #[test]
    fn lacuna_abre_gate() {
        let campos = confronto_de(CONTRATO);
        let item = itens_de_gate(&campos)
            .into_iter()
            .find(|i| i.campo == "segmento")
            .expect("segmento e lacuna e deveria abrir gate");
        assert_eq!(item.tipo, TipoDeGate::Lacuna);
    }

    /// O contrato afirma algo que o catalogo nao consegue reproduzir.
    #[test]
    fn declaracao_que_o_catalogo_nao_reproduz_e_conflito() {
        let yaml = CONTRATO.replace(
            "      - name: segmento\n        logicalType: string",
            "      - name: segmento\n        logicalType: string\n        classification: restricted\n        tags:\n          - pii",
        );
        let campos = confronto_de(&yaml);
        let item = itens_de_gate(&campos)
            .into_iter()
            .find(|i| i.campo == "segmento")
            .unwrap();
        assert_eq!(item.tipo, TipoDeGate::Conflito);
    }

    // --- O hash da aprovacao -------------------------------------------------

    /// A aprovacao vale para um conteudo. Mudou o que se pede, o gate fecha de
    /// novo — e nao ha passe permanente.
    #[test]
    fn hash_muda_quando_o_pedido_muda() {
        let base = itens_de_gate(&confronto_de(CONTRATO));
        let yaml = CONTRATO.replace(
            "      - name: cpf\n        logicalType: string",
            "      - name: cpf\n        logicalType: string\n        classification: public",
        );
        let outro = itens_de_gate(&confronto_de(&yaml));
        assert_ne!(hash_do_gate(&base), hash_do_gate(&outro));
    }

    #[test]
    fn hash_e_estavel_para_o_mesmo_pedido() {
        let a = itens_de_gate(&confronto_de(CONTRATO));
        let b = itens_de_gate(&confronto_de(CONTRATO));
        assert_eq!(hash_do_gate(&a), hash_do_gate(&b));
    }

    // --- O enriquecimento ----------------------------------------------------

    #[test]
    fn campo_classificado_recebe_os_tres_campos_odcs() {
        let (yaml, nao_aplicados) = aplicar(CONTRATO, &laudo().campos).unwrap();
        assert!(nao_aplicados.is_empty(), "{nao_aplicados:?}");

        let d = declaracao_do_yaml(&yaml).unwrap();
        assert_eq!(d["cpf"].classification.as_deref(), Some("restricted"));
        assert_eq!(d["cpf"].pii, Some(true));
        assert_eq!(d["cpf"].sensivel, Some(false));
        assert!(yaml.contains("glossary/glossario.yaml#pessoa.cpf"));
        assert!(yaml.contains("businessDefinition"));
    }

    /// Nao-PII nao ganha tag nenhuma — `tags: []` seria ruido no contrato.
    #[test]
    fn campo_nao_pii_fica_sem_tags() {
        let (yaml, _) = aplicar(CONTRATO, &laudo().campos).unwrap();
        let d = declaracao_do_yaml(&yaml).unwrap();
        assert_eq!(
            d["data_cadastro"].classification.as_deref(),
            Some("internal")
        );
        assert_eq!(d["data_cadastro"].pii, Some(false));
    }

    /// O harness nao escreve "nao sei" no contrato.
    #[test]
    fn campo_sem_classificacao_nao_e_tocado() {
        let (yaml, _) = aplicar(CONTRATO, &laudo().campos).unwrap();
        assert!(declaracao_do_yaml(&yaml).unwrap()["segmento"].vazia());
    }

    /// Rodar F4 de novo nao pode gerar diff.
    #[test]
    fn enriquecimento_e_idempotente() {
        let (uma, _) = aplicar(CONTRATO, &laudo().campos).unwrap();
        let (duas, _) = aplicar(&uma, &laudo().campos).unwrap();
        assert_eq!(uma, duas);
    }

    /// `finance` nao e nossa para remover; `pii` e.
    #[test]
    fn tag_alheia_e_preservada() {
        let yaml = CONTRATO.replace(
            "      - name: cpf\n        logicalType: string",
            "      - name: cpf\n        logicalType: string\n        tags:\n          - finance",
        );
        let (enriquecido, _) = aplicar(&yaml, &laudo().campos).unwrap();
        assert!(enriquecido.contains("finance"));
        assert!(enriquecido.contains("pii"));
    }

    /// Classificacao sem propriedade correspondente e decisao perdida em
    /// silencio — `verify` reprova por isto.
    #[test]
    fn campo_ausente_do_yaml_e_reportado() {
        let sem_cpf = CONTRATO.replace("      - name: cpf\n        logicalType: string\n", "");
        let (_, nao_aplicados) = aplicar(&sem_cpf, &laudo().campos).unwrap();
        assert_eq!(nao_aplicados, vec!["cpf".to_string()]);
    }

    #[test]
    fn contrato_quebrado_e_erro_e_nao_pass_silencioso() {
        assert!(aplicar("::: nao sou yaml :::", &laudo().campos).is_err());
    }

    // --- Determinismo do artefato ---------------------------------------------

    #[test]
    fn mesma_entrada_produz_a_mesma_proposta() {
        let l = laudo();
        let d = declaracao_do_yaml(CONTRATO).unwrap();
        let montar = || {
            let campos = confrontar(&l.campos, &d);
            let gate = itens_de_gate(&campos);
            Proposta {
                contrato: "x".to_string(),
                contrato_sha256: "sha".to_string(),
                glossario_versao: "1.0.0".to_string(),
                catalogo_versao: "1.0.0".to_string(),
                resumo: resumir(&campos),
                gate_sha256: hash_do_gate(&gate),
                gate,
                campos,
            }
        };
        let a = serializar(&montar()).unwrap();
        assert_eq!(a, serializar(&montar()).unwrap());
        assert!(!a.contains("run_id"), "artefato nao pode carregar run_id");
    }

    // --- Contra os artefatos do repositorio -----------------------------------

    /// O confronto para o contrato, o glossario e o catalogo que estao no
    /// repositorio, com o catalogo passado como parametro para que um teste
    /// possa simular a mudanca de uma entrada sem editar o arquivo.
    ///
    /// Os nomes dos campos saem de `declaracao_do_yaml`, e nao do
    /// `export jsonschema`: aqui nao ha container, e as chaves que a leitura
    /// devolve **sao** as propriedades do contrato.
    fn repositorio_com(cat: &Catalogo) -> Vec<CampoDoGate> {
        let contrato = include_str!("../../contracts/clientes/contract.odcs.yaml");
        let g = carregar_glossario(include_str!("../../glossary/glossario.yaml")).unwrap();

        let declarados = declaracao_do_yaml(contrato).unwrap();
        let campos: Vec<Campo> = declarados
            .keys()
            .map(|n| Campo {
                nome: n.clone(),
                tipo: "string".to_string(),
            })
            .collect();

        let m = mapear(ALVO, &campos, &g, "sha-c", "sha-g");
        let l = classificar(&m, cat, "sha-cat");
        confrontar(&l.campos, &declarados)
    }

    fn catalogo_do_repositorio() -> Catalogo {
        carregar_catalogo(include_str!("../../classification/catalogo-lgpd.yaml")).unwrap()
    }

    /// O repositorio nao pode carregar divergencia pendente: com o contrato e o
    /// catalogo como estao, o unico gate legitimo e o de lacuna — campo que o
    /// vocabulario nao cobre.
    #[test]
    fn repositorio_nao_tem_reclassificacao_nem_conflito_pendente() {
        let itens = itens_de_gate(&repositorio_com(&catalogo_do_repositorio()));
        assert!(
            itens.iter().all(|i| i.tipo == TipoDeGate::Lacuna),
            "so lacuna e esperada aqui: {itens:?}"
        );
    }

    /// A prova de que o gate protege o contrato real, e nao so um YAML de
    /// teste: baixar `pessoa.cpf` no catalogo — o *major* previsto em
    /// `classification/README.md` — abre gate sem ninguem avisar o harness.
    #[test]
    fn despromover_o_catalogo_abre_gate_no_contrato_do_repositorio() {
        let mut cat = catalogo_do_repositorio();
        let cpf = cat
            .classificacoes
            .iter_mut()
            .find(|e| e.termo == "pessoa.cpf")
            .expect("o catalogo do repositorio classifica pessoa.cpf");
        cpf.classification = crate::features::f3_classificar::Nivel::Internal;
        cpf.pii = false;

        let item = itens_de_gate(&repositorio_com(&cat))
            .into_iter()
            .find(|i| i.campo == "cpf")
            .expect("despromover cpf tem de abrir gate");
        assert_eq!(item.tipo, TipoDeGate::Reclassificacao);
        assert!(item.detalhe.contains("restricted"), "{}", item.detalhe);
    }

    #[test]
    fn resumo_bate_com_as_decisoes() {
        let r = resumir(&confronto_de(CONTRATO));
        assert_eq!(r.campos, 3);
        assert_eq!(r.primeira_classificacao, 2);
        assert_eq!(r.classificados, 2);
        assert_eq!(r.lacunas, 1);
        assert_eq!(r.reclassificacoes, 0);
        assert_eq!(r.conflitos, 0);
    }

    /// Todo campo classificado no contrato real tem base legal. E a condicao
    /// para o laudo gerado substituir o que o time escreve a mao: sem o artigo,
    /// o documento e uma tabela de conclusoes.
    #[test]
    fn todo_campo_classificado_do_repositorio_tem_base_legal() {
        for c in repositorio_com(&catalogo_do_repositorio()) {
            if c.proposto.classification.is_none() {
                continue;
            }
            assert!(
                c.referencia
                    .as_deref()
                    .is_some_and(|r| !r.trim().is_empty()),
                "campo `{}` classificado sem base legal",
                c.campo
            );
            assert!(
                !c.justificativa.trim().is_empty(),
                "campo `{}` classificado sem justificativa",
                c.campo
            );
        }
    }

    // --- A base legal atravessando para F4 -------------------------------------

    /// O que F3 leu do catalogo tem de chegar inteiro a F4. Enquanto nao
    /// chegava, `f4-proposta.json` saia com a conclusao e sem o criterio.
    #[test]
    fn justificativa_e_base_legal_atravessam_para_o_confronto() {
        let campos = confronto_de(CONTRATO);
        let cpf = campos.iter().find(|c| c.campo == "cpf").unwrap();
        assert_eq!(cpf.referencia.as_deref(), Some("LGPD art. 5, I"));
        assert!(cpf.justificativa.contains("Identificador univoco"));
    }

    /// Lacuna nao tem artigo de lei: tem ausencia de vocabulario.
    #[test]
    fn campo_sem_classificacao_nao_ganha_base_legal() {
        let campos = confronto_de(CONTRATO);
        let seg = campos.iter().find(|c| c.campo == "segmento").unwrap();
        assert_eq!(seg.referencia, None);
        assert!(seg.justificativa.contains("decisao humana"));
    }

    /// O pedido de gate e sobre o **que** se decide, nao sobre a redacao do
    /// porque. Se a base legal entrasse no hash, corrigir uma virgula na
    /// justificativa de um termo invalidaria aprovacoes que ninguem pediu para
    /// revisar.
    #[test]
    fn base_legal_nao_entra_no_hash_do_gate() {
        let antes = hash_do_gate(&itens_de_gate(&confronto_de(CONTRATO)));

        let mut campos = confronto_de(CONTRATO);
        for c in &mut campos {
            c.justificativa = "redacao completamente diferente".to_string();
            c.referencia = Some("outra lei".to_string());
        }
        assert_eq!(antes, hash_do_gate(&itens_de_gate(&campos)));
    }

    // --- O laudo ----------------------------------------------------------------

    fn proposta_de(yaml: &str) -> (Proposta, Laudo) {
        let l = laudo();
        let campos = confrontar(&l.campos, &declaracao_do_yaml(yaml).unwrap());
        let gate = itens_de_gate(&campos);
        let p = Proposta {
            contrato: ALVO.to_string(),
            contrato_sha256: l.contrato_sha256.clone(),
            glossario_versao: l.glossario_versao.clone(),
            catalogo_versao: l.catalogo_versao.clone(),
            resumo: resumir(&campos),
            gate_sha256: hash_do_gate(&gate),
            gate,
            campos,
        };
        (p, l)
    }

    #[test]
    fn laudo_traz_a_base_legal_de_cada_campo_classificado() {
        let (p, l) = proposta_de(CONTRATO);
        let doc = documento_do_laudo(&p, &l, "1.0.0", "abcdef1234567890");

        assert!(doc.contains("Base legal"), "{doc}");
        assert!(doc.contains("LGPD art. 5, I"), "{doc}");
        assert!(
            doc.contains("Identificador univoco de pessoa natural."),
            "{doc}"
        );
    }

    /// O laudo nomeia o criterio, e nao so o resultado: quem audita pergunta
    /// qual catalogo classificou este campo.
    #[test]
    fn laudo_nomeia_glossario_e_catalogo_com_versao() {
        let (p, l) = proposta_de(CONTRATO);
        let doc = documento_do_laudo(&p, &l, "1.0.0", "abcdef1234567890");
        assert!(doc.contains("Criterio aplicado"));
        assert!(doc.contains("classification/catalogo-lgpd.yaml"), "{doc}");
        assert!(doc.contains("glossary/glossario.yaml"), "{doc}");
    }

    /// A lacuna aparece no laudo, nomeada. Um laudo que so lista o que deu
    /// certo esconde exatamente o que o revisor precisa ver.
    #[test]
    fn laudo_lista_as_pendencias() {
        let (p, l) = proposta_de(CONTRATO);
        let doc = documento_do_laudo(&p, &l, "1.0.0", "abcdef1234567890");
        assert!(doc.contains("Pendencias de decisao humana"));
        assert!(doc.contains("segmento"), "{doc}");
        assert!(doc.contains("lacuna"), "{doc}");
    }

    /// Registro que muda sozinho nao serve como registro. A data e a do commit,
    /// entao nao pode haver carimbo no corpo — e reemitir nao pode gerar diff.
    #[test]
    fn laudo_e_deterministico_e_sem_run_id() {
        let (p, l) = proposta_de(CONTRATO);
        let a = documento_do_laudo(&p, &l, "1.0.0", "abcdef1234567890");
        let b = documento_do_laudo(&p, &l, "1.0.0", "abcdef1234567890");
        assert_eq!(a, b);
        assert!(!a.contains("run_id"), "laudo nao pode carregar run_id");
    }

    /// A justificativa vem do catalogo, escrita por gente. Um `|` no meio dela
    /// quebraria a tabela inteira, em silencio.
    #[test]
    fn celula_escapa_pipe_e_achata_quebra_de_linha() {
        assert_eq!(celula("a | b"), "a \\| b");
        assert_eq!(celula("linha um\n   linha dois"), "linha um linha dois");
    }

    #[test]
    fn curto_nao_entra_em_panico_com_hash_pequeno() {
        assert_eq!(curto("abc"), "abc");
        assert_eq!(curto("0123456789abcdef00"), "0123456789abcdef");
    }

    // --- Onde o laudo e arquivado -----------------------------------------------

    #[test]
    fn laudo_fica_ao_lado_do_contrato_com_versao_e_sha_no_nome() {
        assert_eq!(
            caminho_do_laudo(ALVO, "1.2.0", "abcdef1234567890"),
            "contracts/clientes/laudos/1.2.0-abcdef1.md"
        );
    }

    /// Mesma versao reclassificada por um catalogo novo sao duas constatacoes
    /// diferentes. O sha no nome e o que impede a segunda de apagar a primeira.
    #[test]
    fn contratos_diferentes_na_mesma_versao_nao_colidem() {
        assert_ne!(
            caminho_do_laudo(ALVO, "1.0.0", "aaaaaaa1111"),
            caminho_do_laudo(ALVO, "1.0.0", "bbbbbbb2222")
        );
    }

    #[test]
    fn versao_do_yaml_aceita_string_e_numero() {
        assert_eq!(versao_do_yaml("version: 1.0.0\n").unwrap(), "1.0.0");
        let n = versao_do_yaml("version: 1.0\n").unwrap();
        assert!(n.starts_with('1'), "{n}");
    }

    #[test]
    fn contrato_sem_versao_nao_emite_laudo() {
        let e = versao_do_yaml("id: x\n").unwrap_err();
        assert!(format!("{e:#}").contains("version"), "{e:#}");
    }

    /// A versao vira nome de arquivo. `1.0/2` criaria um diretorio.
    #[test]
    fn versao_que_nao_serve_de_nome_de_arquivo_e_recusada() {
        assert!(versao_do_yaml("version: \"1.0/2\"\n").is_err());
        assert!(versao_do_yaml("version: \"1.0 rc\"\n").is_err());
    }

    /// O contrato do repositorio tem de conseguir emitir laudo — se a versao
    /// dele nao servisse de nome de arquivo, o fluxo quebraria so em producao.
    #[test]
    fn o_contrato_do_repositorio_emite_laudo() {
        let contrato = include_str!("../../contracts/clientes/contract.odcs.yaml");
        let v = versao_do_yaml(contrato).unwrap();
        assert!(!v.is_empty());
        assert!(caminho_do_laudo(ALVO, &v, "abcdef1234567890").ends_with(".md"));
    }
}

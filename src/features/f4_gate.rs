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
use crate::features::f3_classificar::{self, CampoClassificado, Catalogo, Laudo, Situacao};
use crate::flow::Outcome;
use crate::phases::Run;
use crate::state::{Aprovacoes, GatePendente, SCHEMA_VERSION};
use crate::tools;
use crate::trace;
use anyhow::{Context, Result};
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

    aplicar_no_contrato(run, &c)
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

    let path = run.cfg.root.join(contrato::CAMINHO);
    let bruto = fs::read_to_string(&path).map_err(|e| {
        format!(
            "contrato `{}` ilegivel em {} ({e})",
            contrato::CAMINHO,
            path.display()
        )
    })?;

    let declarados = declaracao_do_yaml(&bruto).map_err(|e| format!("{e:#}"))?;
    let campos = confrontar(&laudo.campos, &declarados);
    let gate = itens_de_gate(&campos);
    let (yaml_enriquecido, nao_aplicados) =
        aplicar(&bruto, &laudo.campos).map_err(|e| format!("{e:#}"))?;

    let proposta = Proposta {
        contrato: contrato::CAMINHO.to_string(),
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
    let destino = run.cfg.root.join(contrato::CAMINHO);
    let atual = fs::read_to_string(&destino).unwrap_or_default();

    if atual == c.yaml_enriquecido {
        run.note(format!(
            "contrato {} ja reflete a classificacao vigente — nada a escrever",
            contrato::CAMINHO
        ));
        return Outcome::Pass;
    }

    if let Err(e) = fs::write(&destino, &c.yaml_enriquecido) {
        return Outcome::Fail(format!("escrevendo {}: {e}", contrato::CAMINHO));
    }
    run.note(format!(
        "contrato {} enriquecido — {} campo(s) marcado(s); o diff vai no commit do handoff",
        contrato::CAMINHO,
        c.proposta.resumo.classificados
    ));
    Outcome::Pass
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
        let m = mapear(&campos, &glossario(), "sha-c", "sha-g");
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

        let m = mapear(&campos, &g, "sha-c", "sha-g");
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
}

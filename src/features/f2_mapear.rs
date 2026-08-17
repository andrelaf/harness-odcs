//! F2 — Mapear: cada campo do contrato e casado com um termo do glossario
//! canonico, ou nomeado como lacuna.
//!
//! Spec da feature: `docs/spec-f2-mapear.md`.
//!
//! Divisao entre as fases: `implement` **produz** o mapeamento e o grava como
//! proposta; `verify` **refaz o mapeamento do zero** e julga integridade e
//! cobertura. Verify nao le a proposta como insumo — recalcula a partir do
//! contrato e do glossario, que e o que faz `./run.sh verify` valer sozinho.
//!
//! Recalcular tem um efeito de graca: quando as duas fases rodaram no mesmo
//! run, a comparacao byte a byte entre proposta e recomputacao so pode
//! divergir se uma das entradas mudou no meio do run. E a pergunta que o hash
//! do contrato responde em F1, respondida dentro de um unico run.
//!
//! Nenhum modelo decide nada aqui. O casamento e deterministico: normaliza o
//! nome do campo e procura a chave. Ambiguidade nao e resolvida, e **nomeada**
//! — vira lacuna e segue para o humano em F4.

use crate::features::contrato::{self, Campo};
use crate::flow::Outcome;
use crate::phases::Run;
use crate::tools;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

/// O glossario nao mora em `contracts/` porque nao e um contrato: e o
/// vocabulario contra o qual todos os contratos sao lidos. Ver
/// `glossary/README.md`.
const GLOSSARIO: &str = "glossary/glossario.yaml";

// --- Fases -------------------------------------------------------------------

/// Produz o mapeamento e o grava como proposta.
pub fn implement(run: &mut Run) -> Outcome {
    let mapeamento = match mapeamento_atual(run, "f2", "implement") {
        Ok(m) => m,
        Err(e) => return Outcome::Fail(e),
    };

    let destino = format!("evidence/{}/f2-mapeamento.json", run.tracer.run_id());
    let serializado = match serializar(&mapeamento) {
        Ok(s) => s,
        Err(e) => return Outcome::Fail(format!("serializando o mapeamento: {e:#}")),
    };
    if let Err(e) = fs::write(run.cfg.root.join(&destino), &serializado) {
        return Outcome::Fail(format!("escrevendo {destino}: {e}"));
    }

    let r = &mapeamento.resumo;
    run.note(format!(
        "{} campo(s) — {} mapeado(s), {} lacuna(s) — proposta em {destino}",
        r.campos, r.mapeados, r.lacunas
    ));
    Outcome::Pass
}

/// Refaz o mapeamento e julga integridade e cobertura.
pub fn verify(run: &mut Run) -> Outcome {
    let campos = match contrato::extrair(run, "f2", "verify") {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(e),
    };
    let (glossario, gloss_sha) = match glossario_do_disco(run) {
        Ok(g) => g,
        Err(e) => return Outcome::Fail(e),
    };
    let contrato_sha = match contrato::sha(run) {
        Ok(s) => s,
        Err(e) => return Outcome::Fail(e),
    };

    let mut defeitos = defeitos_do_glossario(&glossario);
    let mapeamento = mapear(
        &run.contrato.clone(),
        &campos,
        &glossario,
        &contrato_sha,
        &gloss_sha,
    );
    defeitos.extend(conferir_cobertura(&campos, &mapeamento, &glossario));

    // A comparacao com a proposta de `implement` so existe quando as duas
    // fases rodaram no mesmo run. Rodando `verify` sozinho nao ha com o que
    // comparar — e a nota diz isso, em vez de omitir.
    let proposta = run.evidence_dir.join("f2-mapeamento.json");
    let conferido = match (proposta.exists(), serializar(&mapeamento)) {
        (false, _) => {
            run.note("sem proposta de `implement` neste run — nada a conferir".to_string());
            false
        }
        (true, Err(e)) => {
            defeitos.push(format!("serializando o mapeamento: {e:#}"));
            false
        }
        (true, Ok(recomputado)) => match fs::read_to_string(&proposta) {
            Ok(gravado) if gravado == recomputado => {
                run.note("recomputacao bate com a proposta de `implement`".to_string());
                true
            }
            Ok(_) => {
                defeitos.push(
                    "recomputacao difere da proposta de `implement` — alguma entrada mudou \
                     no meio do run"
                        .to_string(),
                );
                false
            }
            Err(e) => {
                defeitos.push(format!("lendo a proposta de `implement`: {e}"));
                false
            }
        },
    };

    let veredito = Veredito {
        aprovado: defeitos.is_empty(),
        conferido_contra_implement: conferido,
        glossario_versao: mapeamento.glossario_versao.clone(),
        resumo: mapeamento.resumo.clone(),
        lacunas: mapeamento
            .campos
            .iter()
            .filter(|d| d.decisao == Decisao::SemCorrespondencia)
            .map(|d| d.campo.clone())
            .collect(),
        defeitos: defeitos.clone(),
    };
    if let Err(e) = gravar_veredito(run, &veredito) {
        return Outcome::Fail(format!("{e:#}"));
    }

    let r = &mapeamento.resumo;
    run.note(format!(
        "cobertura {}/{} campo(s) decidido(s) — {} mapeado(s), {} lacuna(s), glossario {}",
        r.campos, r.campos, r.mapeados, r.lacunas, mapeamento.glossario_versao
    ));

    if !defeitos.is_empty() {
        for d in &defeitos {
            run.note(format!("  defeito — {d}"));
        }
        return Outcome::Fail(defeitos.join("; "));
    }

    // Lacuna nao reprova: cobertura total aqui e de decisao, nao de acerto. O
    // relatorio para o humano e F4 — nomear os campos e o que F2 deve.
    if !veredito.lacunas.is_empty() {
        run.note(format!(
            "lacuna(s) para F4: {}",
            veredito.lacunas.join(", ")
        ));
        run.risco(format!(
            "{} campo(s) sem termo no glossario: {}",
            veredito.lacunas.len(),
            veredito.lacunas.join(", ")
        ));
    }

    relatorio_legivel(run, &mapeamento)
}

/// A tabela que uma pessoa le sem ferramenta. Nasce em `verify`, depois do
/// veredito, pelo mesmo motivo de F1: relatorio e comprovacao do julgamento,
/// nao insumo dele.
fn relatorio_legivel(run: &mut Run, m: &Mapeamento) -> Outcome {
    let destino = format!("evidence/{}/f2-mapeamento.md", run.tracer.run_id());
    if let Err(e) = fs::write(run.cfg.root.join(&destino), markdown(m)) {
        return Outcome::Fail(format!("escrevendo {destino}: {e}"));
    }
    run.note(format!("relatorio {destino}"));
    Outcome::Pass
}

// --- Montagem, com I/O --------------------------------------------------------

/// Contrato + glossario -> mapeamento. As duas fases de F2 passam por aqui, e
/// F3 tambem: e assim que a classificacao chega ao termo canonico sem depender
/// da evidencia de um run anterior. Evidencia e saida, nunca entrada.
///
/// `feature` entra no nome do artefato de evidencia porque quem chama nem
/// sempre e F2.
pub fn mapeamento_atual(run: &mut Run, feature: &str, fase: &str) -> Result<Mapeamento, String> {
    let campos = contrato::extrair(run, feature, fase)?;
    let (glossario, gloss_sha) = glossario_do_disco(run)?;

    // Entrada inutilizavel para na preparacao: com alias colidindo entre dois
    // termos nao existe mapeamento a produzir, porque o harness nao escolhe
    // qual dos dois vale. Resultado ruim e que e julgado em `verify`.
    let defeitos = defeitos_do_glossario(&glossario);
    if !defeitos.is_empty() {
        return Err(format!(
            "glossario `{GLOSSARIO}` invalido: {}",
            defeitos.join("; ")
        ));
    }

    let contrato_sha = contrato::sha(run)?;
    run.note(format!(
        "glossario {} v{} — {} termo(s), sha256 {}",
        GLOSSARIO,
        glossario.version,
        glossario.termos.len(),
        &gloss_sha[..16]
    ));
    Ok(mapear(
        &run.contrato.clone(),
        &campos,
        &glossario,
        &contrato_sha,
        &gloss_sha,
    ))
}

/// O glossario do disco, com o sha256 do que foi lido. Publica porque F3
/// tambem precisa saber contra qual vocabulario esta trabalhando.
pub fn glossario_do_disco(run: &Run) -> Result<(Glossario, String), String> {
    let path = run.cfg.vocab.join(GLOSSARIO);
    let bruto = fs::read_to_string(&path).map_err(|e| {
        format!(
            "glossario `{GLOSSARIO}` ilegivel em {} ({e})",
            path.display()
        )
    })?;
    let glossario = carregar_glossario(&bruto).map_err(|e| format!("{e:#}"))?;
    Ok((glossario, tools::sha256_hex(&bruto)))
}

fn gravar_veredito(run: &mut Run, v: &Veredito) -> Result<()> {
    let destino = format!("evidence/{}/f2-cobertura.json", run.tracer.run_id());
    let corpo = serde_json::to_string_pretty(v).context("serializando o veredito")?;
    fs::write(run.cfg.root.join(&destino), corpo)
        .with_context(|| format!("escrevendo {destino}"))?;
    run.note(format!("veredito {destino}"));
    Ok(())
}

// --- Glossario ----------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Glossario {
    pub version: String,
    #[serde(default)]
    pub termos: Vec<Termo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Termo {
    pub id: String,
    #[serde(default)]
    pub nome: String,
    #[serde(default)]
    pub definicao: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

impl Termo {
    /// As chaves pelas quais este termo casa: o proprio `id` mais cada alias,
    /// todos normalizados.
    fn chaves(&self) -> Vec<String> {
        std::iter::once(&self.id)
            .chain(self.aliases.iter())
            .map(|s| normalizar(s))
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// Funcao pura: YAML em glossario.
pub fn carregar_glossario(bruto: &str) -> Result<Glossario> {
    serde_norway::from_str(bruto).context("glossario nao e YAML valido no formato esperado")
}

/// O que torna o glossario inutilizavel. Lista vazia = integro.
///
/// Regras em `glossary/README.md`. Termo que nenhum contrato usa **nao** entra
/// aqui: o glossario e da organizacao e existe antes do contrato que o consome.
pub fn defeitos_do_glossario(g: &Glossario) -> Vec<String> {
    let mut defeitos = Vec::new();

    if g.version.trim().is_empty() {
        defeitos.push("glossario sem `version`".to_string());
    }
    if g.termos.is_empty() {
        defeitos.push("glossario sem nenhum termo".to_string());
    }

    // BTreeMap para que a ordem dos defeitos seja estavel entre execucoes —
    // uma mensagem de FAIL que muda de ordem nao serve para comparar runs.
    let mut dono: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for t in &g.termos {
        let id = t.id.trim();
        if id.is_empty() {
            defeitos.push("termo sem `id`".to_string());
            continue;
        }
        if t.nome.trim().is_empty() {
            defeitos.push(format!("termo `{id}` sem `nome`"));
        }
        if t.definicao.trim().is_empty() {
            defeitos.push(format!("termo `{id}` sem `definicao`"));
        }

        let chaves = t.chaves();
        if chaves.is_empty() {
            defeitos.push(format!("termo `{id}` sem nenhuma chave utilizavel"));
        }
        for chave in chaves {
            let donos = dono.entry(chave).or_default();
            // Repetir a mesma chave dentro do proprio termo tambem conta: o
            // alias duplicado nao muda o casamento, mas denuncia edicao
            // descuidada do arquivo.
            donos.push(id.to_string());
        }
    }

    for (chave, donos) in &dono {
        if donos.len() > 1 {
            defeitos.push(format!(
                "chave `{chave}` declarada por mais de um termo ({}) — o harness nao \
                 escolhe sozinho qual vale",
                donos.join(", ")
            ));
        }
    }

    defeitos
}

/// Chave normalizada -> termo. So faz sentido com o glossario integro; com
/// chave colidindo, um dos donos venceria em silencio.
fn indice(g: &Glossario) -> BTreeMap<String, &Termo> {
    let mut m = BTreeMap::new();
    for t in &g.termos {
        for chave in t.chaves() {
            m.entry(chave).or_insert(t);
        }
    }
    m
}

/// Minusculas, e tudo que nao for letra ou digito vira `_`, com repeticoes
/// colapsadas e bordas aparadas.
///
/// Acento **nao** e normalizado, de proposito: casar `codigo_postal` com
/// `codigo postal` e reconhecer o mesmo separador escrito de outro jeito;
/// casar com `código_postal` seria adivinhar uma grafia que ninguem declarou.
/// Variacao de escrita se resolve acrescentando o alias — decisao declarada,
/// nao inferida.
/// O ultimo segmento de um caminho: `cliente.cpf` -> `cpf`,
/// `entregas[].cep` -> `cep`, `cpf` -> `cpf`.
///
/// O `[]` sai junto porque ele marca travessia de array, e nao faz parte do
/// nome do campo. Sem isso, `entregas[]` de um array de escalares nunca casaria
/// com termo nenhum.
pub fn folha(caminho: &str) -> &str {
    caminho
        .rsplit('.')
        .next()
        .unwrap_or(caminho)
        .trim_end_matches("[]")
}

/// Minusculas, e tudo que nao for letra ou digito vira `_`, com repeticoes
/// colapsadas e bordas aparadas.
pub fn normalizar(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

// --- Mapeamento ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Decisao {
    Mapeado,
    SemCorrespondencia,
}

/// A decisao sobre um campo. `regra` e o identificador estavel para quem
/// audita por maquina; `justificativa` e a frase para quem audita lendo —
/// `contexto.md` exige justificativa por campo, e um enum sozinho nao e uma.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Decidido {
    pub campo: String,
    pub tipo: String,
    pub decisao: Decisao,
    pub termo: Option<String>,
    pub regra: String,
    pub justificativa: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resumo {
    pub campos: usize,
    pub mapeados: usize,
    pub lacunas: usize,
}

/// A proposta de mapeamento.
///
/// **Sem `run_id` e sem timestamp**, deliberadamente: o mesmo contrato com o
/// mesmo glossario produz o mesmo arquivo em qualquer run, e e isso que torna
/// dois runs comparaveis com `diff` — e que permite a `verify` conferir a
/// recomputacao byte a byte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mapeamento {
    pub contrato: String,
    pub contrato_sha256: String,
    pub glossario: String,
    pub glossario_versao: String,
    pub glossario_sha256: String,
    pub resumo: Resumo,
    pub campos: Vec<Decidido>,
}

/// Funcao pura: campos + glossario -> uma decisao por campo.
///
/// `contrato` vem por parametro, e nao de uma constante, porque o repositorio
/// tem N contratos: o artefato precisa dizer a qual deles se refere.
pub fn mapear(
    contrato: &str,
    campos: &[Campo],
    glossario: &Glossario,
    contrato_sha: &str,
    glossario_sha: &str,
) -> Mapeamento {
    let idx = indice(glossario);

    let decididos: Vec<Decidido> = campos
        .iter()
        .map(|c| {
            // Caminho inteiro primeiro, folha depois. O primeiro permite ao
            // glossario desambiguar quando o mesmo nome significa coisas
            // diferentes em ramos diferentes; o segundo e o que faz
            // `cliente.cpf` casar com `pessoa.cpf` sem ninguem cadastrar
            // caminho nenhum.
            let chave = normalizar(&c.nome);
            let achado = idx
                .get(&chave)
                .or_else(|| idx.get(&normalizar(folha(&c.nome))));
            match achado {
                Some(t) => Decidido {
                    campo: c.nome.clone(),
                    tipo: c.tipo.clone(),
                    decisao: Decisao::Mapeado,
                    termo: Some(t.id.clone()),
                    regra: "chave_exata".to_string(),
                    justificativa: format!(
                        "`{}` normalizado e `{chave}`, que o termo `{}` declara",
                        c.nome, t.id
                    ),
                },
                None => Decidido {
                    campo: c.nome.clone(),
                    tipo: c.tipo.clone(),
                    decisao: Decisao::SemCorrespondencia,
                    termo: None,
                    regra: "sem_chave".to_string(),
                    justificativa: format!(
                        "nenhum termo do glossario declara a chave `{chave}` — lacuna \
                         para decisao humana"
                    ),
                },
            }
        })
        .collect();

    let mapeados = decididos
        .iter()
        .filter(|d| d.decisao == Decisao::Mapeado)
        .count();

    Mapeamento {
        contrato: contrato.to_string(),
        contrato_sha256: contrato_sha.to_string(),
        glossario: GLOSSARIO.to_string(),
        glossario_versao: glossario.version.clone(),
        glossario_sha256: glossario_sha.to_string(),
        resumo: Resumo {
            campos: decididos.len(),
            mapeados,
            lacunas: decididos.len() - mapeados,
        },
        campos: decididos,
    }
}

/// Funcao pura: o que impede o mapeamento de ser aceito. Lista vazia = passa.
///
/// Cobertura aqui e de **decisao**, nao de acerto — campo sem termo e lacuna,
/// nao defeito. O que nao pode existir e campo que atravessou o fluxo sem
/// ninguem dizer nada sobre ele, que e a primeira falha previsivel listada em
/// `contexto.md`.
pub fn conferir_cobertura(campos: &[Campo], m: &Mapeamento, g: &Glossario) -> Vec<String> {
    let mut defeitos = Vec::new();

    let mut vistos: BTreeMap<&str, usize> = BTreeMap::new();
    for d in &m.campos {
        *vistos.entry(d.campo.as_str()).or_insert(0) += 1;
    }

    for c in campos {
        match vistos.get(c.nome.as_str()) {
            None => defeitos.push(format!("campo `{}` do contrato ficou sem decisao", c.nome)),
            Some(1) => {}
            Some(n) => defeitos.push(format!("campo `{}` decidido {n} vezes", c.nome)),
        }
    }
    for nome in vistos.keys() {
        if !campos.iter().any(|c| c.nome == *nome) {
            defeitos.push(format!(
                "mapeamento decide `{nome}`, que nao existe no contrato"
            ));
        }
    }

    let ids: Vec<&str> = g.termos.iter().map(|t| t.id.as_str()).collect();
    for d in &m.campos {
        match (d.decisao, d.termo.as_deref()) {
            (Decisao::Mapeado, None) => {
                defeitos.push(format!("campo `{}` marcado mapeado sem termo", d.campo));
            }
            (Decisao::Mapeado, Some(t)) if !ids.contains(&t) => {
                defeitos.push(format!(
                    "campo `{}` aponta para o termo `{t}`, que nao existe no glossario",
                    d.campo
                ));
            }
            (Decisao::SemCorrespondencia, Some(t)) => {
                defeitos.push(format!(
                    "campo `{}` marcado como lacuna mas aponta para `{t}`",
                    d.campo
                ));
            }
            _ => {}
        }
        if d.justificativa.trim().is_empty() {
            defeitos.push(format!("campo `{}` decidido sem justificativa", d.campo));
        }
    }

    let mapeados = m
        .campos
        .iter()
        .filter(|d| d.decisao == Decisao::Mapeado)
        .count();
    let esperado = Resumo {
        campos: m.campos.len(),
        mapeados,
        lacunas: m.campos.len() - mapeados,
    };
    if m.resumo != esperado {
        defeitos.push(format!(
            "resumo nao bate com as decisoes: diz {}/{}/{}, as decisoes dao {}/{}/{}",
            m.resumo.campos,
            m.resumo.mapeados,
            m.resumo.lacunas,
            esperado.campos,
            esperado.mapeados,
            esperado.lacunas
        ));
    }

    defeitos
}

// --- Veredito e relatorio ------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Veredito {
    pub aprovado: bool,
    pub conferido_contra_implement: bool,
    pub glossario_versao: String,
    pub resumo: Resumo,
    pub lacunas: Vec<String>,
    pub defeitos: Vec<String>,
}

/// Serializacao unica: `implement` grava com ela e `verify` recomputa com ela.
/// Duas rotas de serializacao fariam a comparacao byte a byte acusar
/// divergencia por diferenca de formatacao.
fn serializar(m: &Mapeamento) -> Result<String> {
    serde_json::to_string_pretty(m).context("serializando o mapeamento")
}

fn markdown(m: &Mapeamento) -> String {
    let mut s = String::new();
    s.push_str("# F2 — Mapeamento campo -> glossario\n\n");
    s.push_str(&format!(
        "- Contrato: `{}` (sha256 `{}`)\n",
        m.contrato,
        &m.contrato_sha256[..16]
    ));
    s.push_str(&format!(
        "- Glossario: `{}` v{} (sha256 `{}`)\n",
        m.glossario,
        m.glossario_versao,
        &m.glossario_sha256[..16]
    ));
    s.push_str(&format!(
        "- Cobertura: {} campo(s) — {} mapeado(s), {} lacuna(s)\n\n",
        m.resumo.campos, m.resumo.mapeados, m.resumo.lacunas
    ));
    s.push_str("| Campo | Tipo | Decisao | Termo | Justificativa |\n");
    s.push_str("|---|---|---|---|---|\n");
    for d in &m.campos {
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            d.campo,
            // `|` na celula quebraria a tabela, e o tipo opcional sai do motor
            // como `string|null`.
            d.tipo.replace('|', "\\|"),
            match d.decisao {
                Decisao::Mapeado => "mapeado",
                Decisao::SemCorrespondencia => "**lacuna**",
            },
            d.termo
                .as_deref()
                .map(|t| format!("`{t}`"))
                .unwrap_or_else(|| "—".to_string()),
            d.justificativa
        ));
    }
    s.push_str(
        "\nLacuna nao reprova F2: a cobertura exigida e de decisao, nao de acerto. \
         O relatorio de lacunas para decisao humana e F4.\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O contrato dos testes. Antes era `contrato::CAMINHO`; agora que o alvo e
    /// resolvido por run, o valor entra por parametro tambem aqui.
    const ALVO: &str = "contracts/clientes/contract.odcs.yaml";

    fn glossario_exemplo() -> Glossario {
        carregar_glossario(
            r#"
version: 1.0.0
termos:
  - id: pessoa.cpf
    nome: CPF
    definicao: Numero de inscricao no CPF.
    aliases: [cpf, nr_cpf]
  - id: contato.email
    nome: E-mail
    definicao: Endereco de correio eletronico.
    aliases: [email]
"#,
        )
        .unwrap()
    }

    fn campo(nome: &str) -> Campo {
        Campo {
            nome: nome.to_string(),
            tipo: "string".to_string(),
        }
    }

    #[test]
    fn separadores_colapsam_e_bordas_somem() {
        assert_eq!(normalizar("Nome Completo"), "nome_completo");
        assert_eq!(normalizar("pessoa.cpf"), "pessoa_cpf");
        assert_eq!(normalizar("__nr--cpf__"), "nr_cpf");
    }

    /// Acento nao e normalizado de proposito: casar por aproximacao criaria
    /// vinculo que ninguem declarou.
    #[test]
    fn acento_nao_e_normalizado() {
        assert_ne!(normalizar("codigo_postal"), normalizar("código_postal"));
    }

    #[test]
    fn glossario_do_repositorio_e_integro() {
        let bruto = include_str!("../../glossary/glossario.yaml");
        let g = carregar_glossario(bruto).expect("glossario do repositorio nao parseia");
        assert!(
            defeitos_do_glossario(&g).is_empty(),
            "{:?}",
            defeitos_do_glossario(&g)
        );
    }

    #[test]
    fn alias_colidindo_entre_termos_e_defeito() {
        let g = carregar_glossario(
            r#"
version: 1.0.0
termos:
  - id: pessoa.cpf
    nome: CPF
    definicao: x
    aliases: [documento]
  - id: pessoa.rg
    nome: RG
    definicao: y
    aliases: [documento]
"#,
        )
        .unwrap();
        let d = defeitos_do_glossario(&g);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("documento"));
    }

    /// Alias de um termo igual ao `id` de outro e o mesmo problema, escrito de
    /// outro jeito — e por isso o `id` tambem entra no espaco de chaves.
    #[test]
    fn alias_colidindo_com_id_de_outro_termo_e_defeito() {
        let g = carregar_glossario(
            r#"
version: 1.0.0
termos:
  - id: pessoa.cpf
    nome: CPF
    definicao: x
  - id: documento
    nome: Documento
    definicao: y
    aliases: [pessoa.cpf]
"#,
        )
        .unwrap();
        assert!(!defeitos_do_glossario(&g).is_empty());
    }

    #[test]
    fn termo_sem_definicao_e_defeito() {
        let g = carregar_glossario(
            r#"
version: 1.0.0
termos:
  - id: pessoa.cpf
    nome: CPF
    definicao: ""
"#,
        )
        .unwrap();
        assert!(defeitos_do_glossario(&g)[0].contains("definicao"));
    }

    /// Termo que nenhum contrato usa nao e defeito: o glossario e da
    /// organizacao e existe antes do contrato que o consome.
    #[test]
    fn termo_nao_usado_nao_e_defeito() {
        let g = glossario_exemplo();
        let m = mapear(ALVO, &[campo("cpf")], &g, "sha-c", "sha-g");
        assert!(defeitos_do_glossario(&g).is_empty());
        assert!(conferir_cobertura(&[campo("cpf")], &m, &g).is_empty());
    }

    #[test]
    fn campo_casa_por_alias_e_por_id() {
        let g = glossario_exemplo();
        let campos = vec![campo("nr_cpf"), campo("contato.email")];
        let m = mapear(ALVO, &campos, &g, "sha-c", "sha-g");
        assert_eq!(m.resumo.mapeados, 2);
        assert_eq!(m.campos[0].termo.as_deref(), Some("pessoa.cpf"));
        assert_eq!(m.campos[1].termo.as_deref(), Some("contato.email"));
    }

    #[test]
    fn campo_sem_termo_vira_lacuna_com_justificativa() {
        let g = glossario_exemplo();
        let campos = vec![campo("cpf"), campo("segmento")];
        let m = mapear(ALVO, &campos, &g, "sha-c", "sha-g");

        assert_eq!(
            m.resumo,
            Resumo {
                campos: 2,
                mapeados: 1,
                lacunas: 1
            }
        );
        let lacuna = &m.campos[1];
        assert_eq!(lacuna.decisao, Decisao::SemCorrespondencia);
        assert!(lacuna.termo.is_none());
        assert!(lacuna.justificativa.contains("segmento"));
        // Lacuna nao e defeito: o relatorio para o humano e F4.
        assert!(conferir_cobertura(&campos, &m, &g).is_empty());
    }

    #[test]
    fn campo_esquecido_e_defeito_de_cobertura() {
        let g = glossario_exemplo();
        let campos = vec![campo("cpf"), campo("email")];
        let mut m = mapear(ALVO, &campos, &g, "sha-c", "sha-g");
        m.campos.pop();
        m.resumo = Resumo {
            campos: 1,
            mapeados: 1,
            lacunas: 0,
        };

        let d = conferir_cobertura(&campos, &m, &g);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("email") && d[0].contains("sem decisao"));
    }

    #[test]
    fn campo_decidido_duas_vezes_e_defeito_de_cobertura() {
        let g = glossario_exemplo();
        let campos = vec![campo("cpf")];
        let mut m = mapear(ALVO, &campos, &g, "sha-c", "sha-g");
        m.campos.push(m.campos[0].clone());

        let d = conferir_cobertura(&campos, &m, &g);
        assert!(d.iter().any(|x| x.contains("decidido 2 vezes")));
    }

    #[test]
    fn termo_fora_do_glossario_e_defeito_de_cobertura() {
        let g = glossario_exemplo();
        let campos = vec![campo("cpf")];
        let mut m = mapear(ALVO, &campos, &g, "sha-c", "sha-g");
        m.campos[0].termo = Some("pessoa.inventado".to_string());

        let d = conferir_cobertura(&campos, &m, &g);
        assert!(d.iter().any(|x| x.contains("nao existe no glossario")));
    }

    #[test]
    fn resumo_que_nao_bate_e_defeito_de_cobertura() {
        let g = glossario_exemplo();
        let campos = vec![campo("cpf"), campo("segmento")];
        let mut m = mapear(ALVO, &campos, &g, "sha-c", "sha-g");
        m.resumo.mapeados = 2;

        let d = conferir_cobertura(&campos, &m, &g);
        assert!(d.iter().any(|x| x.contains("resumo nao bate")));
    }

    /// A comparacao byte a byte de `verify` so faz sentido se a serializacao
    /// do mesmo insumo for identica.
    #[test]
    fn mesma_entrada_produz_o_mesmo_arquivo() {
        let g = glossario_exemplo();
        let campos = vec![campo("cpf"), campo("segmento")];
        let a = serializar(&mapear(ALVO, &campos, &g, "sha-c", "sha-g")).unwrap();
        let b = serializar(&mapear(ALVO, &campos, &g, "sha-c", "sha-g")).unwrap();
        assert_eq!(a, b);
        assert!(!a.contains("run_id"), "artefato nao pode carregar run_id");
    }
}

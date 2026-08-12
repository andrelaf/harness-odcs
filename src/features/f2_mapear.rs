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

use crate::flow::Outcome;
use crate::phases::Run;
use crate::tools;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

/// O mesmo alvo de F1. Caminho relativo a raiz: no host resolve com
/// `root.join(...)`, no container vai como esta, porque a raiz e montada em
/// `/home/datacontract`.
const CONTRATO: &str = "contracts/clientes/contract.odcs.yaml";

/// O glossario nao mora em `contracts/` porque nao e um contrato: e o
/// vocabulario contra o qual todos os contratos sao lidos. Ver
/// `glossary/README.md`.
const GLOSSARIO: &str = "glossary/glossario.yaml";

// --- Fases -------------------------------------------------------------------

/// Produz o mapeamento e o grava como proposta.
pub fn implement(run: &mut Run) -> Outcome {
    let mapeamento = match compor(run, "implement") {
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
    let campos = match extrair_campos(run, "verify") {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(e),
    };
    let (glossario, gloss_sha) = match carregar_do_disco(run) {
        Ok(g) => g,
        Err(e) => return Outcome::Fail(e),
    };
    let contrato_sha = match sha_do_contrato(run) {
        Ok(s) => s,
        Err(e) => return Outcome::Fail(e),
    };

    let mut defeitos = defeitos_do_glossario(&glossario);
    let mapeamento = mapear(&campos, &glossario, &contrato_sha, &gloss_sha);
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

/// Contrato + glossario -> mapeamento. As duas fases passam por aqui, entao
/// nao ha como divergirem no que consideram entrada.
fn compor(run: &mut Run, fase: &str) -> Result<Mapeamento, String> {
    let campos = extrair_campos(run, fase)?;
    let (glossario, gloss_sha) = carregar_do_disco(run)?;

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

    let contrato_sha = sha_do_contrato(run)?;
    run.note(format!(
        "glossario {} v{} — {} termo(s), sha256 {}",
        GLOSSARIO,
        glossario.version,
        glossario.termos.len(),
        &gloss_sha[..16]
    ));
    Ok(mapear(&campos, &glossario, &contrato_sha, &gloss_sha))
}

/// Quem le o contrato e o motor, nao o harness: `datacontract export
/// jsonschema`. Ler `schema[].properties[].name` do YAML aqui criaria uma
/// segunda interpretacao do ODCS no repositorio, e a segunda seria a errada.
fn extrair_campos(run: &mut Run, fase: &str) -> Result<Vec<Campo>, String> {
    if let Err(e) = fs::create_dir_all(&run.evidence_dir) {
        return Err(format!("criando {}: {e}", run.evidence_dir.display()));
    }

    // Um arquivo por fase: os dois lado a lado sao a evidencia de que a
    // extracao se repete. Um destino unico faria a segunda fase apagar a
    // prova da primeira.
    let destino = format!("evidence/{}/f2-campos-{fase}.json", run.tracer.run_id());
    let saida = run
        .datacontract(
            &format!("{fase}-campos"),
            &["export", "jsonschema", CONTRATO, "--output", &destino],
        )
        .map_err(|e| format!("{e}"))?;

    let bruto = fs::read_to_string(run.cfg.root.join(&destino)).map_err(|e| {
        format!(
            "`export jsonschema` saiu com {} e nao deixou {destino} ({e})",
            saida.exit_code
        )
    })?;
    if !saida.ok() {
        return Err(format!("`export jsonschema` saiu com {}", saida.exit_code));
    }

    ler_campos(&bruto).map_err(|e| format!("{e:#}"))
}

fn carregar_do_disco(run: &Run) -> Result<(Glossario, String), String> {
    let path = run.cfg.root.join(GLOSSARIO);
    let bruto = fs::read_to_string(&path).map_err(|e| {
        format!(
            "glossario `{GLOSSARIO}` ilegivel em {} ({e})",
            path.display()
        )
    })?;
    let glossario = carregar_glossario(&bruto).map_err(|e| format!("{e:#}"))?;
    Ok((glossario, tools::sha256_hex(&bruto)))
}

/// Identidade da entrada, pelo mesmo motivo de F1: quando dois runs
/// discordarem, foi o contrato que mudou ou a ferramenta?
fn sha_do_contrato(run: &Run) -> Result<String, String> {
    let path = run.cfg.root.join(CONTRATO);
    fs::read_to_string(&path)
        .map(|s| tools::sha256_hex(&s))
        .map_err(|e| format!("contrato `{CONTRATO}` ilegivel em {} ({e})", path.display()))
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

// --- Campos do contrato --------------------------------------------------------

/// Um campo do contrato, reduzido ao que o mapeamento precisa. So metadado:
/// nome e tipo. Nenhum valor de dado entra aqui — nem poderia, o contrato nao
/// os contem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Campo {
    pub nome: String,
    pub tipo: String,
}

/// Funcao pura: o JSON Schema exportado pelo motor em lista de campos.
///
/// `BTreeMap` fixa a ordem alfabetica sem depender da feature `preserve_order`
/// do `serde_json`. Nao e a ordem do contrato; e uma ordem **estavel**, que e o
/// que o artefato precisa para ser comparavel entre runs.
pub fn ler_campos(bruto: &str) -> Result<Vec<Campo>> {
    let schema: JsonSchema =
        serde_json::from_str(bruto).context("export do motor nao e JSON valido")?;

    // Zero campo nao e contrato vazio, e sinal de que o formato do export
    // mudou. Passar aqui produziria cobertura de 0/0 campos — um PASS que nao
    // prova nada.
    if schema.properties.is_empty() {
        bail!("export do motor nao trouxe nenhuma propriedade — formato inesperado");
    }

    Ok(schema
        .properties
        .into_iter()
        .map(|(nome, prop)| Campo {
            tipo: tipo_legivel(prop.tipo.as_ref()),
            nome,
        })
        .collect())
}

#[derive(Deserialize)]
struct JsonSchema {
    #[serde(default)]
    properties: BTreeMap<String, PropSchema>,
}

#[derive(Deserialize)]
struct PropSchema {
    #[serde(default, rename = "type")]
    tipo: Option<serde_json::Value>,
}

/// `"string"` sai `string`; `["string","null"]` sai `string|null`. O tipo entra
/// no relatorio como contexto para quem le, nunca como criterio de casamento.
fn tipo_legivel(v: Option<&serde_json::Value>) -> String {
    match v {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str())
            .collect::<Vec<_>>()
            .join("|"),
        _ => "desconhecido".to_string(),
    }
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
pub fn mapear(
    campos: &[Campo],
    glossario: &Glossario,
    contrato_sha: &str,
    glossario_sha: &str,
) -> Mapeamento {
    let idx = indice(glossario);

    let decididos: Vec<Decidido> = campos
        .iter()
        .map(|c| {
            let chave = normalizar(&c.nome);
            match idx.get(&chave) {
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
        contrato: CONTRATO.to_string(),
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
        let m = mapear(&[campo("cpf")], &g, "sha-c", "sha-g");
        assert!(defeitos_do_glossario(&g).is_empty());
        assert!(conferir_cobertura(&[campo("cpf")], &m, &g).is_empty());
    }

    #[test]
    fn campo_casa_por_alias_e_por_id() {
        let g = glossario_exemplo();
        let campos = vec![campo("nr_cpf"), campo("contato.email")];
        let m = mapear(&campos, &g, "sha-c", "sha-g");
        assert_eq!(m.resumo.mapeados, 2);
        assert_eq!(m.campos[0].termo.as_deref(), Some("pessoa.cpf"));
        assert_eq!(m.campos[1].termo.as_deref(), Some("contato.email"));
    }

    #[test]
    fn campo_sem_termo_vira_lacuna_com_justificativa() {
        let g = glossario_exemplo();
        let campos = vec![campo("cpf"), campo("segmento")];
        let m = mapear(&campos, &g, "sha-c", "sha-g");

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
        let mut m = mapear(&campos, &g, "sha-c", "sha-g");
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
        let mut m = mapear(&campos, &g, "sha-c", "sha-g");
        m.campos.push(m.campos[0].clone());

        let d = conferir_cobertura(&campos, &m, &g);
        assert!(d.iter().any(|x| x.contains("decidido 2 vezes")));
    }

    #[test]
    fn termo_fora_do_glossario_e_defeito_de_cobertura() {
        let g = glossario_exemplo();
        let campos = vec![campo("cpf")];
        let mut m = mapear(&campos, &g, "sha-c", "sha-g");
        m.campos[0].termo = Some("pessoa.inventado".to_string());

        let d = conferir_cobertura(&campos, &m, &g);
        assert!(d.iter().any(|x| x.contains("nao existe no glossario")));
    }

    #[test]
    fn resumo_que_nao_bate_e_defeito_de_cobertura() {
        let g = glossario_exemplo();
        let campos = vec![campo("cpf"), campo("segmento")];
        let mut m = mapear(&campos, &g, "sha-c", "sha-g");
        m.resumo.mapeados = 2;

        let d = conferir_cobertura(&campos, &m, &g);
        assert!(d.iter().any(|x| x.contains("resumo nao bate")));
    }

    #[test]
    fn campos_saem_do_export_do_motor() {
        let bruto = r#"{
            "type": "object",
            "properties": {
                "cpf": {"type": "string", "description": "x"},
                "cep": {"type": ["string", "null"]}
            }
        }"#;
        let campos = ler_campos(bruto).unwrap();
        // Ordem alfabetica, estavel entre runs.
        assert_eq!(campos[0].nome, "cep");
        assert_eq!(campos[0].tipo, "string|null");
        assert_eq!(campos[1].nome, "cpf");
        assert_eq!(campos[1].tipo, "string");
    }

    /// Zero propriedade daria cobertura 0/0 — um PASS que nao prova nada.
    #[test]
    fn export_sem_propriedade_e_erro_e_nao_pass_vazio() {
        assert!(ler_campos(r#"{"type": "object", "properties": {}}"#).is_err());
    }

    #[test]
    fn export_quebrado_e_erro_e_nao_pass_silencioso() {
        assert!(ler_campos("nao sou json").is_err());
    }

    /// A comparacao byte a byte de `verify` so faz sentido se a serializacao
    /// do mesmo insumo for identica.
    #[test]
    fn mesma_entrada_produz_o_mesmo_arquivo() {
        let g = glossario_exemplo();
        let campos = vec![campo("cpf"), campo("segmento")];
        let a = serializar(&mapear(&campos, &g, "sha-c", "sha-g")).unwrap();
        let b = serializar(&mapear(&campos, &g, "sha-c", "sha-g")).unwrap();
        assert_eq!(a, b);
        assert!(!a.contains("run_id"), "artefato nao pode carregar run_id");
    }
}

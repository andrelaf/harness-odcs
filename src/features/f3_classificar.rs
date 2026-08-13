//! F3 — Classificar: cada campo recebe classificacao de privacidade, ou fica
//! nomeado como nao classificado.
//!
//! Spec da feature: `docs/spec-f3-classificar.md`.
//!
//! A classificacao mora no **termo**, nunca no nome do campo. E o retorno de F2
//! sendo cobrado: `cpf`, `nr_cpf` e `documento_cpf` casam com `pessoa.cpf`, e o
//! catalogo classifica `pessoa.cpf` — entao os tres recebem a mesma resposta em
//! qualquer contrato, em qualquer semana. Classificar por nome de campo
//! recriaria o problema de consistencia que o glossario existe para resolver.
//!
//! Consequencia direta: campo sem termo nao tem como ser classificado. Lacuna
//! de F2 vira `nao_classificado` e segue para o humano em F4.
//!
//! Os campos falam **ODCS**, e nao um vocabulario proprio: `classification` e o
//! campo do padrao (a secao "Classification & Security" do editor), e
//! `pii`/`sensivel` viram `tags: [pii, sensitive]`, que e a convencao do
//! proprio ODCS — nao existe campo `pii` em ODCS 3.1.0. Assim o enriquecimento
//! que F4 vai propor e valido por construcao, sem traducao no meio do caminho.
//!
//! O mapeamento vem de `f2_mapear::mapeamento_atual`, recomputado — nunca de
//! `evidence/` de um run anterior. Evidencia e saida, nunca entrada.

use crate::features::f2_mapear::{self, Mapeamento};
use crate::flow::Outcome;
use crate::phases::Run;
use crate::tools;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

/// O catalogo e separado do glossario porque sao dois donos com cadencias
/// diferentes: o vocabulario e do data steward, a classificacao e do
/// encarregado de dados. Ver `classification/README.md`.
const CATALOGO: &str = "classification/catalogo-lgpd.yaml";

// --- Fases -------------------------------------------------------------------

/// Classifica e grava a proposta.
pub fn implement(run: &mut Run) -> Outcome {
    let laudo = match laudo_atual(run, "f3", "implement") {
        Ok(c) => c.laudo,
        Err(e) => return Outcome::Fail(e),
    };

    let destino = format!("evidence/{}/f3-classificacao.json", run.tracer.run_id());
    let serializado = match serializar(&laudo) {
        Ok(s) => s,
        Err(e) => return Outcome::Fail(format!("serializando a classificacao: {e:#}")),
    };
    if let Err(e) = fs::write(run.cfg.root.join(&destino), &serializado) {
        return Outcome::Fail(format!("escrevendo {destino}: {e}"));
    }

    let r = &laudo.resumo;
    run.note(format!(
        "{} campo(s) — {} classificado(s), {} nao classificado(s); {} pii, {} sensivel \
         — proposta em {destino}",
        r.campos, r.classificados, r.nao_classificados, r.pii, r.sensivel
    ));
    Outcome::Pass
}

/// Refaz a classificacao e julga integridade e cobertura.
pub fn verify(run: &mut Run) -> Outcome {
    let mapeamento = match f2_mapear::mapeamento_atual(run, "f3", "verify") {
        Ok(m) => m,
        Err(e) => return Outcome::Fail(e),
    };
    let (glossario, _) = match f2_mapear::glossario_do_disco(run) {
        Ok(g) => g,
        Err(e) => return Outcome::Fail(e),
    };
    let (catalogo, cat_sha) = match catalogo_do_disco(run) {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(e),
    };

    let mut defeitos = defeitos_do_catalogo(&catalogo, &glossario);
    let laudo = classificar(&mapeamento, &catalogo, &cat_sha);
    defeitos.extend(conferir_cobertura(&mapeamento, &laudo, &catalogo));

    // A comparacao com a proposta de `implement` so existe quando as duas
    // fases rodaram no mesmo run. Rodando `verify` sozinho nao ha com o que
    // comparar — e a nota diz isso, em vez de omitir.
    let proposta = run.evidence_dir.join("f3-classificacao.json");
    let conferido = match (proposta.exists(), serializar(&laudo)) {
        (false, _) => {
            run.note("sem proposta de `implement` neste run — nada a conferir".to_string());
            false
        }
        (true, Err(e)) => {
            defeitos.push(format!("serializando a classificacao: {e:#}"));
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
        glossario_versao: laudo.glossario_versao.clone(),
        catalogo_versao: laudo.catalogo_versao.clone(),
        resumo: laudo.resumo.clone(),
        nao_classificados: laudo
            .campos
            .iter()
            .filter(|c| c.situacao == Situacao::NaoClassificado)
            .map(|c| c.campo.clone())
            .collect(),
        defeitos: defeitos.clone(),
    };
    if let Err(e) = gravar_veredito(run, &veredito) {
        return Outcome::Fail(format!("{e:#}"));
    }

    let r = &laudo.resumo;
    run.note(format!(
        "cobertura {}/{} campo(s) decidido(s) — {} pii, {} sensivel, {} nao classificado(s); \
         niveis {} — catalogo {}",
        r.campos,
        r.campos,
        r.pii,
        r.sensivel,
        r.nao_classificados,
        niveis_legivel(&r.por_nivel),
        laudo.catalogo_versao
    ));

    if !defeitos.is_empty() {
        for d in &defeitos {
            run.note(format!("  defeito — {d}"));
        }
        return Outcome::Fail(defeitos.join("; "));
    }

    if !veredito.nao_classificados.is_empty() {
        run.note(format!(
            "sem classificacao, para F4: {}",
            veredito.nao_classificados.join(", ")
        ));
        run.risco(format!(
            "{} campo(s) sem classificacao, encaminhados ao gate de F4: {}",
            veredito.nao_classificados.len(),
            veredito.nao_classificados.join(", ")
        ));
    }

    relatorio_legivel(run, &laudo)
}

fn relatorio_legivel(run: &mut Run, l: &Laudo) -> Outcome {
    let destino = format!("evidence/{}/f3-classificacao.md", run.tracer.run_id());
    if let Err(e) = fs::write(run.cfg.root.join(&destino), markdown(l)) {
        return Outcome::Fail(format!("escrevendo {destino}: {e}"));
    }
    run.note(format!("relatorio {destino}"));
    Outcome::Pass
}

// --- Montagem, com I/O --------------------------------------------------------

/// A classificacao recomputada, com as duas entradas que julga-la exige.
///
/// Um struct e nao uma tupla porque quem chama e F4, de outro arquivo: `.laudo`
/// e `.catalogo` se leem sozinhos, `.0` e `.2` nao.
pub struct Classificacao {
    pub laudo: Laudo,
    pub catalogo: Catalogo,
    /// Devolvido junto para que F4 nao precise reextrair os campos do contrato
    /// — seria uma segunda chamada ao container por fase, pelo mesmo resultado.
    pub mapeamento: Mapeamento,
}

/// Contrato + glossario + catalogo -> classificacao. As duas fases de F3 passam
/// por aqui, e F4 tambem: e assim que o enriquecimento chega a decisao do
/// catalogo sem depender da evidencia de um run anterior. Mesmo papel que
/// `f2_mapear::mapeamento_atual` cumpre para F3.
pub fn laudo_atual(run: &mut Run, feature: &str, fase: &str) -> Result<Classificacao, String> {
    // Recomputa o mapeamento em vez de ler `evidence/` de um run anterior:
    // assim as garantias de F2 valem dentro de F3, e a cadeia
    // contrato -> termo -> classificacao e reconstruida inteira a cada run.
    let mapeamento = f2_mapear::mapeamento_atual(run, feature, fase)?;
    let (glossario, _) = f2_mapear::glossario_do_disco(run)?;
    let (catalogo, cat_sha) = catalogo_do_disco(run)?;

    // Entrada inutilizavel para na preparacao; resultado ruim e que e julgado
    // em `verify`. Mesma linha de F1 e F2.
    let defeitos = defeitos_do_catalogo(&catalogo, &glossario);
    if !defeitos.is_empty() {
        return Err(format!(
            "catalogo `{CATALOGO}` invalido: {}",
            defeitos.join("; ")
        ));
    }

    run.note(format!(
        "catalogo {} v{} — {} termo(s) classificado(s), sha256 {}",
        CATALOGO,
        catalogo.version,
        catalogo.classificacoes.len(),
        &cat_sha[..16]
    ));
    let laudo = classificar(&mapeamento, &catalogo, &cat_sha);
    Ok(Classificacao {
        laudo,
        catalogo,
        mapeamento,
    })
}

fn catalogo_do_disco(run: &Run) -> Result<(Catalogo, String), String> {
    let path = run.cfg.root.join(CATALOGO);
    let bruto = fs::read_to_string(&path)
        .map_err(|e| format!("catalogo `{CATALOGO}` ilegivel em {} ({e})", path.display()))?;
    let catalogo = carregar_catalogo(&bruto).map_err(|e| format!("{e:#}"))?;
    Ok((catalogo, tools::sha256_hex(&bruto)))
}

fn gravar_veredito(run: &mut Run, v: &Veredito) -> Result<()> {
    let destino = format!("evidence/{}/f3-cobertura.json", run.tracer.run_id());
    let corpo = serde_json::to_string_pretty(v).context("serializando o veredito")?;
    fs::write(run.cfg.root.join(&destino), corpo)
        .with_context(|| format!("escrevendo {destino}"))?;
    run.note(format!("veredito {destino}"));
    Ok(())
}

// --- Catalogo -------------------------------------------------------------------

/// O campo `classification` do ODCS. O padrao deixa a string livre ("can be
/// anything, like confidential, restricted, and public"); aqui ela e fechada
/// num vocabulario controlado, declarado em `classification/README.md`. String
/// livre em campo de classificacao vira `Confidential`, `confidencial` e `CONF`
/// em tres semanas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Nivel {
    Public,
    Internal,
    Confidential,
    Restricted,
}

impl Nivel {
    pub fn rotulo(self) -> &'static str {
        match self {
            Nivel::Public => "public",
            Nivel::Internal => "internal",
            Nivel::Confidential => "confidential",
            Nivel::Restricted => "restricted",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Catalogo {
    pub version: String,
    #[serde(default)]
    pub classificacoes: Vec<Entrada>,
}

/// A classificacao de um termo, na forma em que o ODCS a aceita.
///
/// `classification` e o campo do padrao; `pii` e `sensivel` viram
/// `tags: [pii, sensitive]` no contrato enriquecido — o ODCS 3.1.0 nao tem
/// campo `pii`, e tag e a forma nativa de marcar dado pessoal. Escrever assim
/// faz o enriquecimento que F4 vai propor ser valido por construcao.
#[derive(Debug, Clone, Deserialize)]
pub struct Entrada {
    /// Chaveado por termo do glossario, nunca por nome de campo.
    pub termo: String,
    pub classification: Nivel,
    #[serde(default)]
    pub pii: bool,
    #[serde(default)]
    pub sensivel: bool,
    #[serde(default)]
    pub justificativa: String,
    #[serde(default)]
    pub referencia: String,
}

impl Catalogo {
    fn get(&self, termo: &str) -> Option<&Entrada> {
        self.classificacoes.iter().find(|e| e.termo == termo)
    }
}

/// Funcao pura: YAML em catalogo.
pub fn carregar_catalogo(bruto: &str) -> Result<Catalogo> {
    serde_norway::from_str(bruto).context("catalogo nao e YAML valido no formato esperado")
}

/// O que torna o catalogo inutilizavel. Lista vazia = integro.
///
/// Regras em `classification/README.md`. A que mais importa: o catalogo tem de
/// **cobrir o glossario inteiro**. Termo sem classificacao e manutencao
/// atrasada, nao ambiguidade — tratar isso como lacuna deixaria o catalogo
/// apodrecer em silencio, com os campos saindo "pendentes de decisao humana"
/// para sempre sem nada acusar a origem real do problema.
pub fn defeitos_do_catalogo(c: &Catalogo, g: &f2_mapear::Glossario) -> Vec<String> {
    let mut defeitos = Vec::new();

    if c.version.trim().is_empty() {
        defeitos.push("catalogo sem `version`".to_string());
    }
    if c.classificacoes.is_empty() {
        defeitos.push("catalogo sem nenhuma classificacao".to_string());
    }

    // BTreeMap para que a ordem dos defeitos seja estavel entre execucoes.
    let mut vistos: BTreeMap<&str, usize> = BTreeMap::new();
    let termos_do_glossario: Vec<&str> = g.termos.iter().map(|t| t.id.as_str()).collect();

    for e in &c.classificacoes {
        let termo = e.termo.trim();
        if termo.is_empty() {
            defeitos.push("entrada sem `termo`".to_string());
            continue;
        }
        *vistos.entry(termo).or_insert(0) += 1;

        if e.justificativa.trim().is_empty() {
            defeitos.push(format!("termo `{termo}` classificado sem justificativa"));
        }
        if e.referencia.trim().is_empty() {
            defeitos.push(format!("termo `{termo}` classificado sem referencia legal"));
        }
        if !termos_do_glossario.contains(&termo) {
            defeitos.push(format!(
                "catalogo classifica `{termo}`, que nao existe no glossario"
            ));
        }

        defeitos.extend(incoerencias(e));
    }

    for (termo, n) in &vistos {
        if *n > 1 {
            defeitos.push(format!("termo `{termo}` classificado {n} vezes"));
        }
    }

    for termo in &termos_do_glossario {
        if !vistos.contains_key(termo) {
            defeitos.push(format!(
                "termo `{termo}` esta no glossario e nao tem entrada no catalogo"
            ));
        }
    }

    defeitos
}

/// As combinacoes que nao podem existir entre `classification`, `pii` e
/// `sensivel`. Nao sao gosto: cada uma e uma contradicao que so aparece por
/// edicao descuidada do arquivo.
fn incoerencias(e: &Entrada) -> Vec<String> {
    let t = e.termo.trim();
    let mut v = Vec::new();

    // Dado pessoal sensivel e, antes de tudo, dado pessoal.
    if e.sensivel && !e.pii {
        v.push(format!("termo `{t}` marcado `sensivel` sem ser `pii`"));
    }
    if e.sensivel && e.classification != Nivel::Restricted {
        v.push(format!(
            "termo `{t}` e sensivel mas esta como `{}` — sensivel exige `restricted`",
            e.classification.rotulo()
        ));
    }
    if e.pii && e.classification == Nivel::Public {
        v.push(format!("termo `{t}` e `pii` e esta como `public`"));
    }
    // Restringir por segredo comercial e legitimo, mas nao e decisao de um
    // catalogo de privacidade — se nao e dado pessoal, o motivo mora em outro
    // lugar.
    if !e.pii && e.classification == Nivel::Restricted {
        v.push(format!(
            "termo `{t}` nao e `pii` e esta como `restricted` — restricao por outro motivo \
             nao e decisao deste catalogo"
        ));
    }

    v
}

// --- Classificacao ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Situacao {
    Classificado,
    NaoClassificado,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CampoClassificado {
    pub campo: String,
    pub tipo: String,
    pub termo: Option<String>,
    pub situacao: Situacao,
    /// O campo `classification` do ODCS, quando ha classificacao.
    pub classification: Option<Nivel>,
    /// Vira `tags: [pii]` no contrato enriquecido.
    pub pii: Option<bool>,
    /// Vira `tags: [sensitive]`. E o gatilho de gate de F4.
    pub sensivel: Option<bool>,
    pub justificativa: String,
    pub referencia: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resumo {
    pub campos: usize,
    pub classificados: usize,
    pub nao_classificados: usize,
    pub pii: usize,
    pub sensivel: usize,
    /// Contagem por nivel. `BTreeMap` para ordem estavel — a comparacao byte a
    /// byte de `verify` depende disso.
    pub por_nivel: BTreeMap<String, usize>,
}

/// A proposta de classificacao.
///
/// **Sem `run_id` e sem timestamp**, deliberadamente: o mesmo contrato com o
/// mesmo glossario e o mesmo catalogo produz o mesmo arquivo em qualquer run.
/// E o que torna dois runs comparaveis com `diff` e o que permite a `verify`
/// conferir a recomputacao byte a byte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Laudo {
    pub contrato: String,
    pub contrato_sha256: String,
    pub glossario: String,
    pub glossario_versao: String,
    pub glossario_sha256: String,
    pub catalogo: String,
    pub catalogo_versao: String,
    pub catalogo_sha256: String,
    pub resumo: Resumo,
    pub campos: Vec<CampoClassificado>,
}

/// Funcao pura: mapeamento + catalogo -> uma classificacao por campo.
///
/// A classificacao de um campo e uma **consulta**: termo -> entrada do
/// catalogo. Nao ha inferencia, nao ha heuristica sobre nome de campo, nao ha
/// modelo. O que nao esta no catalogo nao e inventado — e nomeado.
pub fn classificar(m: &Mapeamento, catalogo: &Catalogo, catalogo_sha: &str) -> Laudo {
    let campos: Vec<CampoClassificado> = m
        .campos
        .iter()
        .map(|d| match d.termo.as_deref().and_then(|t| catalogo.get(t)) {
            Some(e) => CampoClassificado {
                campo: d.campo.clone(),
                tipo: d.tipo.clone(),
                termo: d.termo.clone(),
                situacao: Situacao::Classificado,
                classification: Some(e.classification),
                pii: Some(e.pii),
                sensivel: Some(e.sensivel),
                justificativa: e.justificativa.trim().to_string(),
                referencia: Some(e.referencia.trim().to_string()),
            },
            // Sem termo, nao ha o que consultar. Com o catalogo integro este
            // ramo so alcanca lacuna de F2 — campo que o vocabulario nao cobre.
            None => CampoClassificado {
                campo: d.campo.clone(),
                tipo: d.tipo.clone(),
                termo: d.termo.clone(),
                situacao: Situacao::NaoClassificado,
                classification: None,
                pii: None,
                sensivel: None,
                justificativa: match d.termo.as_deref() {
                    Some(t) => format!(
                        "termo `{t}` nao tem entrada no catalogo — classificar exige decisao humana"
                    ),
                    None => "campo sem termo no glossario (lacuna de F2) — classificar exige \
                             decisao humana"
                        .to_string(),
                },
                referencia: None,
            },
        })
        .collect();

    Laudo {
        contrato: m.contrato.clone(),
        contrato_sha256: m.contrato_sha256.clone(),
        glossario: m.glossario.clone(),
        glossario_versao: m.glossario_versao.clone(),
        glossario_sha256: m.glossario_sha256.clone(),
        catalogo: CATALOGO.to_string(),
        catalogo_versao: catalogo.version.clone(),
        catalogo_sha256: catalogo_sha.to_string(),
        resumo: resumir(&campos),
        campos,
    }
}

/// Uma unica rota para o resumo: `classificar` o produz e `conferir_cobertura`
/// o recalcula para comparar. Duas contagens escritas a mao divergiriam.
fn resumir(campos: &[CampoClassificado]) -> Resumo {
    let classificados = campos
        .iter()
        .filter(|c| c.situacao == Situacao::Classificado)
        .count();

    let mut por_nivel: BTreeMap<String, usize> = BTreeMap::new();
    for c in campos.iter().filter_map(|c| c.classification) {
        *por_nivel.entry(c.rotulo().to_string()).or_insert(0) += 1;
    }

    Resumo {
        campos: campos.len(),
        classificados,
        nao_classificados: campos.len() - classificados,
        pii: campos.iter().filter(|c| c.pii == Some(true)).count(),
        sensivel: campos.iter().filter(|c| c.sensivel == Some(true)).count(),
        por_nivel,
    }
}

/// Funcao pura: o que impede a classificacao de ser aceita. Lista vazia = passa.
///
/// Alem da cobertura, confere que cada classificacao **veio do catalogo** — e
/// o que impede F3 de ser so uma contagem, e a trave do `contexto.md` ("nunca
/// decide sozinho o que e persistido") escrita como assercao.
pub fn conferir_cobertura(m: &Mapeamento, l: &Laudo, catalogo: &Catalogo) -> Vec<String> {
    let mut defeitos = Vec::new();

    let mut vistos: BTreeMap<&str, usize> = BTreeMap::new();
    for c in &l.campos {
        *vistos.entry(c.campo.as_str()).or_insert(0) += 1;
    }

    for d in &m.campos {
        match vistos.get(d.campo.as_str()) {
            None => defeitos.push(format!(
                "campo `{}` do contrato ficou sem classificacao nem decisao",
                d.campo
            )),
            Some(1) => {}
            Some(n) => defeitos.push(format!("campo `{}` decidido {n} vezes", d.campo)),
        }
    }
    for nome in vistos.keys() {
        if !m.campos.iter().any(|d| d.campo == *nome) {
            defeitos.push(format!(
                "classificacao decide `{nome}`, que nao existe no contrato"
            ));
        }
    }

    for c in &l.campos {
        match c.situacao {
            Situacao::Classificado => match c.termo.as_deref().and_then(|t| catalogo.get(t)) {
                None => defeitos.push(format!(
                    "campo `{}` marcado classificado sem entrada correspondente no catalogo",
                    c.campo
                )),
                Some(e) => {
                    // A classificacao tem de ser a do catalogo, nao uma que
                    // apareceu no meio do caminho.
                    if c.classification != Some(e.classification)
                        || c.pii != Some(e.pii)
                        || c.sensivel != Some(e.sensivel)
                    {
                        defeitos.push(format!(
                            "campo `{}` diverge do catalogo para o termo `{}`",
                            c.campo, e.termo
                        ));
                    }
                    if c.referencia.as_deref().unwrap_or("").trim().is_empty() {
                        defeitos.push(format!("campo `{}` classificado sem referencia", c.campo));
                    }
                }
            },
            Situacao::NaoClassificado => {
                if c.classification.is_some()
                    || c.pii.is_some()
                    || c.sensivel.is_some()
                    || c.referencia.is_some()
                {
                    defeitos.push(format!(
                        "campo `{}` marcado sem classificacao mas carrega veredito",
                        c.campo
                    ));
                }
            }
        }
        if c.justificativa.trim().is_empty() {
            defeitos.push(format!("campo `{}` decidido sem justificativa", c.campo));
        }
    }

    let esperado = resumir(&l.campos);
    if l.resumo != esperado {
        defeitos.push(format!(
            "resumo nao bate com as decisoes: diz {:?}, as decisoes dao {:?}",
            l.resumo, esperado
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
    pub catalogo_versao: String,
    pub resumo: Resumo,
    pub nao_classificados: Vec<String>,
    pub defeitos: Vec<String>,
}

/// Serializacao unica: `implement` grava com ela e `verify` recomputa com ela.
/// Duas rotas fariam a comparacao byte a byte acusar divergencia por diferenca
/// de formatacao.
fn serializar(l: &Laudo) -> Result<String> {
    serde_json::to_string_pretty(l).context("serializando a classificacao")
}

fn niveis_legivel(por_nivel: &BTreeMap<String, usize>) -> String {
    if por_nivel.is_empty() {
        return "-".to_string();
    }
    por_nivel
        .iter()
        .map(|(n, q)| format!("{q} {n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn sim_nao(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "sim",
        Some(false) => "nao",
        None => "—",
    }
}

fn markdown(l: &Laudo) -> String {
    let mut s = String::new();
    s.push_str("# F3 — Classificacao de privacidade (LGPD)\n\n");
    s.push_str(&format!(
        "- Contrato: `{}` (sha256 `{}`)\n",
        l.contrato,
        &l.contrato_sha256[..16]
    ));
    s.push_str(&format!(
        "- Glossario: `{}` v{} (sha256 `{}`)\n",
        l.glossario,
        l.glossario_versao,
        &l.glossario_sha256[..16]
    ));
    s.push_str(&format!(
        "- Catalogo: `{}` v{} (sha256 `{}`)\n",
        l.catalogo,
        l.catalogo_versao,
        &l.catalogo_sha256[..16]
    ));
    s.push_str(&format!(
        "- Cobertura: {} campo(s) — {} classificado(s), {} sem classificacao\n",
        l.resumo.campos, l.resumo.classificados, l.resumo.nao_classificados
    ));
    s.push_str(&format!(
        "- {} campo(s) PII, {} sensivel; por nivel: {}\n\n",
        l.resumo.pii,
        l.resumo.sensivel,
        niveis_legivel(&l.resumo.por_nivel)
    ));

    s.push_str(
        "| Campo | Termo | classification | pii | sensivel | Referencia | Justificativa |\n",
    );
    s.push_str("|---|---|---|---|---|---|---|\n");
    for c in &l.campos {
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            c.campo,
            c.termo
                .as_deref()
                .map(|t| format!("`{t}`"))
                .unwrap_or_else(|| "—".to_string()),
            match c.classification {
                Some(n) => format!("`{}`", n.rotulo()),
                None => "**sem classificacao**".to_string(),
            },
            sim_nao(c.pii),
            sim_nao(c.sensivel),
            c.referencia.as_deref().unwrap_or("—"),
            c.justificativa
        ));
    }
    s.push_str(
        "\nOs campos seguem o ODCS: `classification` e o campo do padrao, e `pii`/`sensivel` \
         viram `tags: [pii, sensitive]` no contrato enriquecido que F4 vai propor.\n\n\
         Campo sem classificacao nao reprova F3: a cobertura exigida e de decisao, nao de \
         acerto. Sem termo no glossario nao ha o que consultar no catalogo, e decidir isso e \
         do humano — o gate e F4.\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::contrato::Campo;
    use crate::features::f2_mapear::{Glossario, carregar_glossario, mapear};

    fn glossario() -> Glossario {
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

    /// Constroi um catalogo com uma unica entrada alterada, para exercitar as
    /// regras de coerencia sem repetir o YAML inteiro em cada teste.
    fn catalogo_com(cpf: &str) -> Catalogo {
        carregar_catalogo(&format!(
            r#"
version: 1.0.0
classificacoes:
  - termo: pessoa.cpf
{cpf}
    justificativa: x
    referencia: y
  - termo: cadastro.data_criacao
    classification: internal
    pii: false
    sensivel: false
    justificativa: x
    referencia: y
"#
        ))
        .unwrap()
    }

    fn laudo_de(nomes: &[&str]) -> (Mapeamento, Laudo, Catalogo) {
        let campos: Vec<Campo> = nomes
            .iter()
            .map(|n| Campo {
                nome: n.to_string(),
                tipo: "string".to_string(),
            })
            .collect();
        let m = mapear(&campos, &glossario(), "sha-c", "sha-g");
        let cat = catalogo();
        let l = classificar(&m, &cat, "sha-cat");
        (m, l, cat)
    }

    #[test]
    fn catalogo_do_repositorio_cobre_o_glossario_do_repositorio() {
        let g = carregar_glossario(include_str!("../../glossary/glossario.yaml")).unwrap();
        let c = carregar_catalogo(include_str!("../../classification/catalogo-lgpd.yaml")).unwrap();
        let d = defeitos_do_catalogo(&c, &g);
        assert!(d.is_empty(), "{d:?}");
    }

    /// A regra que separa lacuna de defeito: o catalogo tem obrigacao de cobrir
    /// o vocabulario, e a falta e manutencao atrasada, nao ambiguidade.
    #[test]
    fn termo_do_glossario_sem_entrada_no_catalogo_e_defeito() {
        let g = carregar_glossario(
            r#"
version: 1.0.0
termos:
  - id: pessoa.cpf
    nome: CPF
    definicao: x
  - id: cadastro.data_criacao
    nome: Data
    definicao: y
  - id: contato.telefone
    nome: Telefone
    definicao: z
"#,
        )
        .unwrap();
        let d = defeitos_do_catalogo(&catalogo(), &g);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("contato.telefone") && d[0].contains("nao tem entrada"));
    }

    #[test]
    fn catalogo_classificando_termo_inexistente_e_defeito() {
        let mut c = catalogo();
        c.classificacoes.push(Entrada {
            termo: "pessoa.inventado".to_string(),
            classification: Nivel::Confidential,
            pii: true,
            sensivel: false,
            justificativa: "x".to_string(),
            referencia: "y".to_string(),
        });
        let d = defeitos_do_catalogo(&c, &glossario());
        assert!(d.iter().any(|x| x.contains("nao existe no glossario")));
    }

    #[test]
    fn sensivel_sem_pii_e_incoerencia() {
        let d = defeitos_do_catalogo(
            &catalogo_com("    classification: restricted\n    pii: false\n    sensivel: true"),
            &glossario(),
        );
        assert!(d.iter().any(|x| x.contains("sem ser `pii`")), "{d:?}");
    }

    #[test]
    fn sensivel_fora_de_restricted_e_incoerencia() {
        let d = defeitos_do_catalogo(
            &catalogo_com("    classification: confidential\n    pii: true\n    sensivel: true"),
            &glossario(),
        );
        assert!(d.iter().any(|x| x.contains("exige `restricted`")), "{d:?}");
    }

    #[test]
    fn pii_publico_e_incoerencia() {
        let d = defeitos_do_catalogo(
            &catalogo_com("    classification: public\n    pii: true\n    sensivel: false"),
            &glossario(),
        );
        assert!(
            d.iter().any(|x| x.contains("`pii` e esta como `public`")),
            "{d:?}"
        );
    }

    /// Restringir por segredo comercial e legitimo, mas nao e decisao de um
    /// catalogo de privacidade.
    #[test]
    fn nao_pii_restrito_e_incoerencia() {
        let d = defeitos_do_catalogo(
            &catalogo_com("    classification: restricted\n    pii: false\n    sensivel: false"),
            &glossario(),
        );
        assert!(
            d.iter().any(|x| x.contains("nao e decisao deste catalogo")),
            "{d:?}"
        );
    }

    #[test]
    fn entrada_sem_referencia_legal_e_defeito() {
        let mut c = catalogo();
        c.classificacoes[0].referencia = "  ".to_string();
        let d = defeitos_do_catalogo(&c, &glossario());
        assert!(d.iter().any(|x| x.contains("sem referencia legal")));
    }

    /// `classification` e string livre no ODCS; aqui o vocabulario e fechado, e
    /// valor fora dele nao carrega.
    #[test]
    fn nivel_fora_do_vocabulario_nao_carrega() {
        let erro = carregar_catalogo(
            r#"
version: 1.0.0
classificacoes:
  - termo: pessoa.cpf
    classification: sigiloso
    pii: true
    justificativa: x
    referencia: y
"#,
        )
        .unwrap_err();
        assert!(format!("{erro:#}").contains("sigiloso"));
    }

    #[test]
    fn campo_herda_a_classificacao_do_termo() {
        let (_, l, _) = laudo_de(&["cpf", "data_cadastro"]);
        assert_eq!(l.campos[0].campo, "cpf");
        assert_eq!(l.campos[0].classification, Some(Nivel::Restricted));
        assert_eq!(l.campos[0].pii, Some(true));
        assert_eq!(l.campos[1].classification, Some(Nivel::Internal));
        assert_eq!(l.campos[1].pii, Some(false));
        assert_eq!(l.resumo.pii, 1);
        assert_eq!(l.resumo.sensivel, 0);
        assert_eq!(l.resumo.por_nivel.get("restricted"), Some(&1));
        assert_eq!(l.resumo.por_nivel.get("internal"), Some(&1));
    }

    /// Dois nomes diferentes para a mesma coisa recebem a mesma resposta —
    /// que e a razao de o glossario existir.
    #[test]
    fn alias_diferente_recebe_a_mesma_classificacao() {
        let (_, l, _) = laudo_de(&["cpf", "CPF"]);
        assert_eq!(l.campos[0].classification, l.campos[1].classification);
        assert_eq!(l.campos[0].termo, l.campos[1].termo);
    }

    #[test]
    fn lacuna_de_f2_vira_nao_classificado_com_justificativa() {
        let (m, l, cat) = laudo_de(&["cpf", "segmento"]);
        let sem = &l.campos[1];
        assert_eq!(sem.campo, "segmento");
        assert_eq!(sem.situacao, Situacao::NaoClassificado);
        assert!(sem.classification.is_none() && sem.pii.is_none() && sem.referencia.is_none());
        assert!(sem.justificativa.contains("decisao humana"));
        // Nao reprova: o gate e F4.
        assert!(conferir_cobertura(&m, &l, &cat).is_empty());
    }

    /// A trave do contexto.md: a classificacao tem de vir do catalogo.
    #[test]
    fn classificacao_divergente_do_catalogo_e_defeito() {
        let (m, mut l, cat) = laudo_de(&["cpf"]);
        l.campos[0].classification = Some(Nivel::Public);
        let d = conferir_cobertura(&m, &l, &cat);
        assert!(d.iter().any(|x| x.contains("diverge do catalogo")), "{d:?}");
    }

    #[test]
    fn pii_divergente_do_catalogo_e_defeito() {
        let (m, mut l, cat) = laudo_de(&["cpf"]);
        l.campos[0].pii = Some(false);
        let d = conferir_cobertura(&m, &l, &cat);
        assert!(d.iter().any(|x| x.contains("diverge do catalogo")), "{d:?}");
    }

    #[test]
    fn campo_esquecido_e_defeito_de_cobertura() {
        let (m, mut l, cat) = laudo_de(&["cpf", "data_cadastro"]);
        l.campos.pop();
        let d = conferir_cobertura(&m, &l, &cat);
        assert!(d.iter().any(|x| x.contains("data_cadastro")));
    }

    #[test]
    fn campo_decidido_duas_vezes_e_defeito_de_cobertura() {
        let (m, mut l, cat) = laudo_de(&["cpf"]);
        l.campos.push(l.campos[0].clone());
        let d = conferir_cobertura(&m, &l, &cat);
        assert!(d.iter().any(|x| x.contains("decidido 2 vezes")));
    }

    #[test]
    fn nao_classificado_carregando_veredito_e_defeito() {
        let (m, mut l, cat) = laudo_de(&["segmento"]);
        l.campos[0].pii = Some(true);
        let d = conferir_cobertura(&m, &l, &cat);
        assert!(d.iter().any(|x| x.contains("carrega veredito")));
    }

    #[test]
    fn resumo_que_nao_bate_e_defeito_de_cobertura() {
        let (m, mut l, cat) = laudo_de(&["cpf"]);
        l.resumo.pii = 7;
        let d = conferir_cobertura(&m, &l, &cat);
        assert!(d.iter().any(|x| x.contains("resumo nao bate")));
    }

    /// A comparacao byte a byte de `verify` so faz sentido se a serializacao
    /// do mesmo insumo for identica.
    #[test]
    fn mesma_entrada_produz_o_mesmo_arquivo() {
        let (_, a, _) = laudo_de(&["cpf", "segmento"]);
        let (_, b, _) = laudo_de(&["cpf", "segmento"]);
        let sa = serializar(&a).unwrap();
        assert_eq!(sa, serializar(&b).unwrap());
        assert!(!sa.contains("run_id"), "artefato nao pode carregar run_id");
    }
}

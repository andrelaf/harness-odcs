//! O contrato ODCS visto pelas features de dominio.
//!
//! Mora fora de qualquer feature porque F2, F3 e F4 leem o mesmo contrato. Com
//! isto dentro de `f2_mapear`, F3 teria de importar de F2 uma coisa que nao e
//! de F2 — e F4 faria o mesmo, ate ninguem mais saber de quem e o que.
//!
//! Quem interpreta ODCS e o `datacontract-cli`, via `export jsonschema`. O
//! harness nao parseia o padrao: ler `schema[].properties[].name` do YAML
//! parece trivial ate o primeiro contrato com propriedade aninhada, e ai
//! existiriam duas interpretacoes do ODCS no repositorio — a segunda sendo a
//! errada.

use crate::ctx::Ctx;
use crate::tools;
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Onde os contratos moram, e como se chama a fonte dentro de cada um.
///
/// O contrato deixou de ser uma constante: um repositorio de dados tem N
/// contratos, e o harness precisa saber em qual esta trabalhando. O caminho
/// resolvido vive em `Ctx::contrato`, escolhido uma vez por run.
pub const DIRETORIO: &str = "contracts";
pub const ARQUIVO: &str = "contract.odcs.yaml";

/// `contracts/<dominio>/<contrato>/` e o layout mais fundo previsto. O teto
/// existe porque uma varredura sem limite seguiria link simbolico para fora do
/// repositorio.
const PROFUNDIDADE_MAX: usize = 4;

/// Todo contrato do repositorio, em ordem estavel.
pub fn descobrir(root: &Path) -> Result<Vec<String>> {
    let mut achados = Vec::new();
    varrer(&root.join(DIRETORIO), root, 0, &mut achados)?;
    achados.sort();
    Ok(achados)
}

fn varrer(dir: &Path, root: &Path, nivel: usize, out: &mut Vec<String>) -> Result<()> {
    if nivel > PROFUNDIDADE_MAX || !dir.is_dir() {
        return Ok(());
    }
    let mut entradas: Vec<_> = fs::read_dir(dir)
        .with_context(|| format!("lendo {}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    // A ordem de `read_dir` e do sistema de arquivos. Sem ordenar, a mensagem
    // que lista os contratos mudaria de maquina para maquina.
    entradas.sort();

    for p in entradas {
        if p.is_dir() {
            varrer(&p, root, nivel + 1, out)?;
        } else if p.file_name().is_some_and(|n| n == ARQUIVO) {
            out.push(relativo(root, &p));
        }
    }
    Ok(())
}

/// Caminho relativo a raiz, sempre com `/`.
///
/// Uma string, dois usos: no host resolve com `root.join(...)`; no container
/// vai como esta, porque a raiz e montada em `/home/datacontract`, que e o
/// diretorio de trabalho do CLI. Traduzir caminho em dois lugares e onde esse
/// tipo de codigo costuma quebrar entre sistemas.
fn relativo(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Qual contrato este run opera.
///
/// Sem escolha explicita, so resolve quando **nao ha ambiguidade**: um unico
/// contrato no repositorio e o alvo obvio, e exigir a flag ali seria cerimonia.
/// Dois ou mais, o harness recusa e lista — adivinhar qual contrato classificar
/// e a decisao errada para tomar em silencio.
pub fn resolver(root: &Path, escolha: Option<&str>) -> Result<String> {
    if let Some(bruto) = escolha {
        let caminho = bruto.trim().replace('\\', "/");

        // O harness le, classifica e **escreve** neste arquivo. Aceitar caminho
        // arbitrario daria a um argumento de linha de comando o poder de fazer
        // o fluxo escrever em qualquer lugar do disco — capacidade que ele nao
        // precisa ter.
        if !caminho.starts_with(&format!("{DIRETORIO}/")) {
            bail!(
                "`{caminho}` esta fora de `{DIRETORIO}/` — o harness so opera contratos do repositorio"
            );
        }
        if caminho.split('/').any(|s| s == "..") {
            bail!("`{caminho}` sobe de diretorio — recusado");
        }
        if !root.join(&caminho).is_file() {
            bail!("contrato `{caminho}` nao existe");
        }
        return Ok(caminho);
    }

    let achados = descobrir(root)?;
    match achados.len() {
        1 => Ok(achados
            .into_iter()
            .next()
            .expect("acabou de conferir len 1")),
        0 => bail!("nenhum `{ARQUIVO}` encontrado em `{DIRETORIO}/`"),
        _ => bail!(
            "{} contratos no repositorio — escolha um com `--contrato`:\n  {}",
            achados.len(),
            achados.join("\n  ")
        ),
    }
}

// --- A convencao de nome ---------------------------------------------------------
//
// Num repositorio com um contrato o caminho e decoracao. Com duzentos, ele e o
// indice: e por ele que uma pessoa acha o contrato, e e por ele que o
// `CODEOWNERS` roteia a revisao para quem responde por aquele dado. Nome fora do
// padrao quebra as duas coisas em silencio — e so aparece quando alguem precisa
// achar o contrato as pressas.

/// Os dois layouts aceitos, do mais recomendado para o tolerado.
///
/// O nivel de dominio existe por razao mecanica, nao estetica: e o que permite
/// ao `CODEOWNERS` dar a revisao ao time dono do dado. Sem ele, ou uma pessoa
/// aprova tudo, ou o arquivo lista contrato por contrato.
const PROFUNDIDADE_MIN: usize = 2;
const PROFUNDIDADE_REC: usize = 3;

/// Funcao pura: o que o caminho viola na convencao. Lista vazia = bem formado.
///
/// Recebe o caminho relativo a raiz, com `/`, como `resolver` devolve.
pub fn defeitos_do_caminho(caminho: &str) -> Vec<String> {
    let mut defeitos = Vec::new();
    let partes: Vec<&str> = caminho.split('/').filter(|s| !s.is_empty()).collect();

    match partes.first() {
        Some(&DIRETORIO) => {}
        _ => defeitos.push(format!("`{caminho}` nao comeca em `{DIRETORIO}/`")),
    }
    match partes.last() {
        Some(&ARQUIVO) => {}
        Some(outro) => defeitos.push(format!(
            "o arquivo se chama `{outro}` e a convencao e `{ARQUIVO}` — o nome do contrato \
             e o do diretorio, o do arquivo e o papel dele"
        )),
        None => defeitos.push("caminho vazio".to_string()),
    }

    // Segmentos de diretorio entre `contracts/` e o arquivo.
    let dirs = if partes.len() >= 2 {
        &partes[1..partes.len() - 1]
    } else {
        &[][..]
    };

    if dirs.is_empty() {
        defeitos.push(format!(
            "`{caminho}` poe o contrato solto em `{DIRETORIO}/` — cada contrato tem o seu \
             diretorio, porque o laudo mora ao lado dele"
        ));
    } else if partes.len() > PROFUNDIDADE_REC + 1 {
        defeitos.push(format!(
            "`{caminho}` tem {} niveis abaixo de `{DIRETORIO}/` e o maximo e {}: \
             `{DIRETORIO}/<dominio>/<contrato>/{ARQUIVO}`",
            dirs.len(),
            PROFUNDIDADE_REC - 1
        ));
    }

    for seg in dirs {
        if !kebab_case(seg) {
            defeitos.push(format!(
                "`{seg}` nao e kebab-case minusculo — use `[a-z0-9]` separado por `-`, sem \
                 acento, sem espaco, sem maiuscula e sem `_`"
            ));
        }
    }

    defeitos
}

/// O caminho e valido mas nao segue a forma recomendada. Nao reprova nada:
/// e a diferenca entre "esta errado" e "vai doer quando o repositorio crescer".
pub fn avisos_do_caminho(caminho: &str) -> Vec<String> {
    let n = caminho.split('/').filter(|s| !s.is_empty()).count();
    if defeitos_do_caminho(caminho).is_empty() && n == PROFUNDIDADE_MIN + 1 {
        return vec![format!(
            "`{caminho}` nao tem nivel de dominio. Funciona, mas `{DIRETORIO}/<dominio>/\
             <contrato>/{ARQUIVO}` e o que permite ao CODEOWNERS rotear a revisao por dono \
             do dado"
        )];
    }
    Vec::new()
}

/// Funcao pura: a identidade declarada tem de bater com onde o arquivo mora.
///
/// Sem isto, `id: clientes-sintetico` dentro de `contracts/clientes/` passa
/// despercebido — e numa arvore com duzentos contratos, achar "o contrato X"
/// vira `grep`, porque o nome que as ferramentas usam nao e o nome que esta no
/// caminho.
pub fn defeitos_da_identidade(caminho: &str, bruto: &str) -> Vec<String> {
    let partes: Vec<&str> = caminho.split('/').filter(|s| !s.is_empty()).collect();
    let Some(dir) = partes.iter().rev().nth(1) else {
        return vec![format!("`{caminho}` nao tem diretorio de contrato")];
    };

    match escalar_da_raiz(bruto, "id") {
        Err(e) => vec![format!("{e:#}")],
        Ok(None) => vec![format!(
            "contrato sem `id` — e ele que amarra o arquivo ao diretorio `{dir}`"
        )],
        Ok(Some(id)) if id == *dir => Vec::new(),
        Ok(Some(id)) => vec![format!(
            "o contrato declara `id: {id}` e mora em `{dir}/` — os dois tem de ser iguais, \
             senao o nome que as ferramentas usam nao e o nome que esta no caminho"
        )],
    }
}

fn kebab_case(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Um escalar da raiz do contrato, como texto.
///
/// Uma rota so para ler `id`, `version` e afins. Duas leituras do mesmo YAML
/// divergiriam na primeira vez que uma delas tratasse um tipo que a outra nao
/// trata — e `version: 1.0` e numero em YAML, nao string.
pub fn escalar_da_raiz(bruto: &str, chave: &str) -> Result<Option<String>> {
    let doc: serde_norway::Value =
        serde_norway::from_str(bruto).context("contrato nao e YAML valido")?;
    Ok(match doc.get(chave) {
        Some(serde_norway::Value::String(s)) => Some(s.trim().to_string()),
        Some(serde_norway::Value::Number(n)) => Some(n.to_string()),
        _ => None,
    })
}

/// Um campo do contrato, reduzido ao que as features precisam. So metadado:
/// nome e tipo. Nenhum valor de dado entra aqui — nem poderia, o contrato nao
/// os contem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Campo {
    pub nome: String,
    pub tipo: String,
}

/// Roda o motor e devolve os campos.
///
/// O destino da evidencia leva feature **e** fase: os arquivos lado a lado sao
/// a prova de que a extracao se repete. Um destino unico faria a segunda fase
/// apagar a prova da primeira.
pub fn extrair(ctx: &mut Ctx, feature: &str, fase: &str) -> Result<Vec<Campo>, String> {
    if let Err(e) = tools::criar_dir_de_evidencia(&ctx.evidence_dir) {
        return Err(format!("{e:#}"));
    }

    let destino = format!(
        "evidence/{}/{feature}-campos-{fase}.json",
        ctx.tracer.run_id()
    );
    let alvo = ctx.contrato.clone();
    let saida = ctx
        .datacontract(
            &format!("{fase}-campos"),
            &["export", "jsonschema", &alvo, "--output", &destino],
        )
        .map_err(|e| format!("{e}"))?;

    let bruto = fs::read_to_string(ctx.cfg.root.join(&destino)).map_err(|e| {
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

/// Identidade da entrada, pelo mesmo motivo de F1: quando dois runs
/// discordarem, foi o contrato que mudou ou a ferramenta?
pub fn sha(ctx: &Ctx) -> Result<String, String> {
    let path = ctx.cfg.root.join(&ctx.contrato);
    fs::read_to_string(&path)
        .map(|s| tools::sha256_hex(&s))
        .map_err(|e| {
            format!(
                "contrato `{}` ilegivel em {} ({e})",
                ctx.contrato,
                path.display()
            )
        })
}

/// Funcao pura: o JSON Schema exportado pelo motor em lista de campos.
///
/// `BTreeMap` fixa a ordem alfabetica sem depender da feature `preserve_order`
/// do `serde_json`. Nao e a ordem do contrato; e uma ordem **estavel**, que e o
/// que os artefatos precisam para serem comparaveis entre runs.
pub fn ler_campos(bruto: &str) -> Result<Vec<Campo>> {
    let schema: JsonSchema =
        serde_json::from_str(bruto).context("export do motor nao e JSON valido")?;

    // Zero campo nao e contrato vazio, e sinal de que o formato do export
    // mudou. Passar aqui produziria cobertura de 0/0 campos — um PASS que nao
    // prova nada.
    if schema.properties.is_empty() {
        bail!("export do motor nao trouxe nenhuma propriedade — formato inesperado");
    }

    let mut campos = Vec::new();
    coletar(&schema.properties, "", &mut campos);

    // Um export so com containers vazios produziria cobertura de 0/0 campos —
    // um PASS que nao prova nada, pela mesma razao do teste acima.
    if campos.is_empty() {
        bail!("export do motor so trouxe containers sem propriedades — formato inesperado");
    }
    Ok(campos)
}

/// Percorre a arvore e emite **as folhas**, com o caminho ate cada uma.
///
/// Container nao e campo: um objeto ou array com filhos e agrupamento, nao
/// carrega valor para classificar, e reporta-lo criaria uma lacuna fantasma —
/// pior, cadastrar esse nome no glossario cobriria a subarvore inteira com uma
/// classificacao so. Foi o cenario descrito em `docs/cobertura.md`.
///
/// Objeto **sem** filhos continua sendo folha: sem estrutura, ele e um valor.
///
/// `[]` marca travessia de array, e e notacao de leitura — o contrato descreve a
/// forma de todo elemento, nunca a de um elemento especifico.
fn coletar(props: &BTreeMap<String, PropSchema>, prefixo: &str, out: &mut Vec<Campo>) {
    for (nome, p) in props {
        let caminho = format!("{prefixo}{nome}");

        if !p.properties.is_empty() {
            coletar(&p.properties, &format!("{caminho}."), out);
            continue;
        }
        if let Some(itens) = &p.items
            && !itens.properties.is_empty()
        {
            coletar(&itens.properties, &format!("{caminho}[]."), out);
            continue;
        }
        out.push(Campo {
            tipo: tipo_legivel(p.tipo.as_ref()),
            nome: caminho,
        });
    }
}

#[derive(serde::Deserialize)]
struct JsonSchema {
    #[serde(default)]
    properties: BTreeMap<String, PropSchema>,
}

/// Recursivo, porque o JSON Schema exportado pelo motor tambem e. `Box` no
/// `items` quebra o ciclo de tamanho que o compilador nao consegue resolver
/// sozinho.
#[derive(serde::Deserialize)]
struct PropSchema {
    #[serde(default, rename = "type")]
    tipo: Option<serde_json::Value>,
    #[serde(default)]
    properties: BTreeMap<String, PropSchema>,
    #[serde(default)]
    items: Option<Box<PropSchema>>,
}

/// `"string"` sai `string`; `["string","null"]` sai `string|null`. O tipo entra
/// nos relatorios como contexto para quem le, nunca como criterio de decisao.
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- F5: a arvore, e nao so o primeiro nivel -------------------------------

    /// O export do motor para `contracts/pedidos/`, reduzido ao que importa:
    /// objeto aninhado e array de objetos.
    const ANINHADO: &str = r#"{
      "properties": {
        "_id": {"type": "string"},
        "cliente": {"type": "object", "properties": {
          "cpf": {"type": "string"},
          "email": {"type": ["string","null"]}
        }},
        "entregas": {"type": ["array","null"], "items": {"type": "object", "properties": {
          "cep": {"type": "string"}
        }}}
      }
    }"#;

    /// O caso que existia antes de F5: cinco nos vistos, e os de dado pessoal
    /// entre os invisiveis. Agora as folhas aparecem com o caminho ate elas.
    #[test]
    fn desce_em_objeto_e_em_array() {
        let campos = ler_campos(ANINHADO).unwrap();
        let nomes: Vec<&str> = campos.iter().map(|c| c.nome.as_str()).collect();
        assert_eq!(
            nomes,
            ["_id", "cliente.cpf", "cliente.email", "entregas[].cep"]
        );
    }

    /// Container nao e campo. Reporta-lo criaria lacuna fantasma — e cadastrar
    /// esse nome no glossario cobriria a subarvore inteira com uma
    /// classificacao so, que e o pior caminho descrito em `docs/cobertura.md`.
    #[test]
    fn container_nao_entra_como_campo() {
        let campos = ler_campos(ANINHADO).unwrap();
        for proibido in ["cliente", "entregas"] {
            assert!(
                !campos.iter().any(|c| c.nome == proibido),
                "`{proibido}` e agrupamento, nao campo"
            );
        }
    }

    /// Objeto declarado sem filhos e valor, nao agrupamento: sem estrutura, nao
    /// ha o que descer.
    #[test]
    fn objeto_sem_filhos_continua_sendo_campo() {
        let campos = ler_campos(r#"{"properties":{"payload":{"type":"object"}}}"#).unwrap();
        assert_eq!(campos.len(), 1);
        assert_eq!(campos[0].nome, "payload");
    }

    /// Array de escalares nao tem folha para descer — o array **e** o campo.
    #[test]
    fn array_de_escalares_e_um_campo_so() {
        let campos =
            ler_campos(r#"{"properties":{"tags":{"type":"array","items":{"type":"string"}}}}"#)
                .unwrap();
        assert_eq!(campos.len(), 1);
        assert_eq!(campos[0].nome, "tags");
    }

    /// Export so com container vazio produziria cobertura de 0/0 campos — um
    /// PASS que nao prova nada.
    #[test]
    fn arvore_sem_folha_nenhuma_reprova() {
        let r = ler_campos(r#"{"properties":{"vazio":{"type":"object","properties":{}}}}"#);
        assert!(r.is_ok(), "objeto sem filhos e folha");
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

    // --- A convencao de nome -------------------------------------------------

    const RECOMENDADO: &str = "contracts/clientes/cadastro/contract.odcs.yaml";

    #[test]
    fn caminho_recomendado_nao_tem_defeito_nem_aviso() {
        assert!(defeitos_do_caminho(RECOMENDADO).is_empty());
        assert!(avisos_do_caminho(RECOMENDADO).is_empty());
    }

    /// Sem nivel de dominio funciona, mas custa o roteamento por CODEOWNERS —
    /// entao e aviso, e nao reprovacao.
    #[test]
    fn caminho_sem_dominio_passa_com_aviso() {
        let c = "contracts/cadastro/contract.odcs.yaml";
        assert!(
            defeitos_do_caminho(c).is_empty(),
            "{:?}",
            defeitos_do_caminho(c)
        );
        let avisos = avisos_do_caminho(c);
        assert_eq!(avisos.len(), 1, "{avisos:?}");
        assert!(avisos[0].contains("dominio"));
    }

    /// O laudo mora ao lado do contrato, entao o contrato precisa de um
    /// diretorio proprio — solto em `contracts/` nao ha "ao lado".
    #[test]
    fn contrato_solto_em_contracts_e_defeito() {
        let d = defeitos_do_caminho("contracts/contract.odcs.yaml");
        assert!(d.iter().any(|x| x.contains("solto")), "{d:?}");
    }

    #[test]
    fn mais_fundo_que_dominio_e_contrato_e_defeito() {
        let d = defeitos_do_caminho("contracts/a/b/c/contract.odcs.yaml");
        assert!(d.iter().any(|x| x.contains("niveis")), "{d:?}");
    }

    #[test]
    fn arquivo_com_outro_nome_e_defeito() {
        let d = defeitos_do_caminho("contracts/clientes/clientes.odcs.yaml");
        assert!(d.iter().any(|x| x.contains(ARQUIVO)), "{d:?}");
    }

    #[test]
    fn fora_de_contracts_e_defeito() {
        let d = defeitos_do_caminho("glossary/clientes/contract.odcs.yaml");
        assert!(d.iter().any(|x| x.contains("nao comeca")), "{d:?}");
    }

    /// Maiuscula, acento, espaco e `_` sao os quatro jeitos de escrever o mesmo
    /// nome de quatro formas diferentes — que e o problema que a convencao
    /// existe para evitar.
    #[test]
    fn segmento_fora_de_kebab_case_e_defeito() {
        for ruim in [
            "contracts/Clientes/contract.odcs.yaml",
            "contracts/cadastro_clientes/contract.odcs.yaml",
            "contracts/cadastro clientes/contract.odcs.yaml",
            "contracts/endereço/contract.odcs.yaml",
            "contracts/clientes--x/contract.odcs.yaml",
            "contracts/-clientes/contract.odcs.yaml",
            "contracts/clientes-/contract.odcs.yaml",
        ] {
            let d = defeitos_do_caminho(ruim);
            assert!(
                d.iter().any(|x| x.contains("kebab-case")),
                "`{ruim}` deveria reprovar: {d:?}"
            );
        }
    }

    #[test]
    fn kebab_case_aceita_digito_e_hifen_simples() {
        assert!(kebab_case("clientes"));
        assert!(kebab_case("cadastro-pf"));
        assert!(kebab_case("base-2024"));
        assert!(!kebab_case(""));
    }

    // --- A identidade --------------------------------------------------------

    #[test]
    fn id_igual_ao_diretorio_passa() {
        let d = defeitos_da_identidade(
            "contracts/clientes/cadastro/contract.odcs.yaml",
            "id: cadastro\nversion: 1.0.0\n",
        );
        assert!(d.is_empty(), "{d:?}");
    }

    /// O caso real que este check existe para pegar: o contrato se chama uma
    /// coisa e mora em outra.
    #[test]
    fn id_diferente_do_diretorio_e_defeito() {
        let d = defeitos_da_identidade(
            "contracts/clientes/contract.odcs.yaml",
            "id: clientes-sintetico\n",
        );
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("clientes-sintetico") && d[0].contains("clientes/"));
    }

    #[test]
    fn contrato_sem_id_e_defeito() {
        let d = defeitos_da_identidade("contracts/clientes/contract.odcs.yaml", "version: 1.0.0\n");
        assert!(d.iter().any(|x| x.contains("sem `id`")), "{d:?}");
    }

    #[test]
    fn escalar_da_raiz_le_string_e_numero() {
        assert_eq!(
            escalar_da_raiz("id: clientes\n", "id").unwrap().as_deref(),
            Some("clientes")
        );
        // `version: 1.0` e numero em YAML.
        assert!(
            escalar_da_raiz("version: 1.0\n", "version")
                .unwrap()
                .is_some()
        );
        assert_eq!(escalar_da_raiz("id: x\n", "ausente").unwrap(), None);
    }

    // --- Contra o repositorio ------------------------------------------------

    /// O contrato do repositorio tem de seguir a propria convencao. Se este
    /// teste reprova, o repositorio e que esta errado — nao a regra.
    #[test]
    fn o_contrato_do_repositorio_segue_a_convencao() {
        let caminho = "contracts/clientes/contract.odcs.yaml";
        let bruto = include_str!("../../../../contracts/clientes/contract.odcs.yaml");

        let mut d = defeitos_do_caminho(caminho);
        d.extend(defeitos_da_identidade(caminho, bruto));
        assert!(d.is_empty(), "{d:?}");
    }
}

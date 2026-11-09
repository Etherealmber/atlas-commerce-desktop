//! Client situation from the worker's existing printable report.
//! Parse it into inert native text and tables: no scripts, external resources,
//! report tickets, session credentials in URLs, or recalculated balances.
use atlas_api::{ApiClient, ApiError};
use scraper::{ElementRef, Html, Selector};

#[derive(Clone, Debug)]
pub(crate) enum ReportBlock {
    Text(String),
    Table(Vec<Vec<String>>),
}
#[derive(Clone, Debug)]
pub(crate) struct ClientReport {
    pub blocks: Vec<ReportBlock>,
}

fn skipped(element: &ElementRef<'_>) -> bool {
    matches!(
        element.value().name(),
        "script" | "style" | "noscript" | "iframe" | "object" | "embed" | "svg"
    ) || element
        .value()
        .classes()
        .any(|name| matches!(name, "actions" | "sign-row"))
}
fn text(element: ElementRef<'_>) -> String {
    fn walk(element: ElementRef<'_>, parts: &mut Vec<String>) {
        if skipped(&element) {
            return;
        }
        for child in element.children() {
            if let Some(el) = ElementRef::wrap(child) {
                walk(el, parts);
            } else if let Some(node) = child.value().as_text() {
                let value = node.trim();
                if !value.is_empty() {
                    parts.push(value.to_owned());
                }
            }
        }
    }
    let mut parts = Vec::new();
    walk(element, &mut parts);
    parts
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn collect(
    element: ElementRef<'_>,
    blocks: &mut Vec<ReportBlock>,
    row_sel: &Selector,
    cell_sel: &Selector,
) {
    if skipped(&element) {
        return;
    }
    let tag = element.value().name();
    if tag == "table" {
        let rows: Vec<Vec<String>> = element
            .select(row_sel)
            .map(|row| row.select(cell_sel).map(text).collect())
            .collect();
        blocks.push(ReportBlock::Table(rows));
    } else if matches!(tag, "p" | "h1" | "h2" | "h3" | "th" | "td")
        || element.value().classes().any(|c| c == "stat")
    {
        let content = text(element);
        if !content.is_empty() {
            blocks.push(ReportBlock::Text(content));
        }
    } else {
        let children: Vec<_> = element.children().filter_map(ElementRef::wrap).collect();
        if children.is_empty() {
            let content = text(element);
            if !content.is_empty() {
                blocks.push(ReportBlock::Text(content));
            }
        } else {
            for child in children {
                collect(child, blocks, row_sel, cell_sel);
            }
        }
    }
}
fn parse_report(html: &str) -> Result<ClientReport, ApiError> {
    let doc = Html::parse_document(html);
    let selector = |s: &str| Selector::parse(s).expect("static selector");
    let sheet = doc
        .select(&selector(".sheet"))
        .next()
        .ok_or_else(|| ApiError::client("Format du rapport client non reconnu."))?;
    let title = sheet
        .select(&selector("h1"))
        .next()
        .map(text)
        .unwrap_or_default();
    if !title.starts_with("Situation client") || sheet.select(&selector("table")).count() < 2 {
        return Err(ApiError::client("Format du rapport client non reconnu."));
    }
    let mut blocks = Vec::new();
    collect(sheet, &mut blocks, &selector("tr"), &selector("th, td"));
    Ok(ClientReport { blocks })
}

pub(crate) async fn fetch(
    api: &ApiClient,
    client: &atlas_api::models::Client,
    full: bool,
) -> Result<ClientReport, ApiError> {
    if client.id <= 0 {
        return Err(ApiError::client("Identifiant client invalide."));
    }
    let token = api
        .token()
        .ok_or_else(|| ApiError::client("Session indisponible."))?;
    let mut url = url::Url::parse(&format!("{}/api/reports/client-situation", api.base_url()))
        .map_err(|_| ApiError::client("Adresse du rapport invalide."))?;
    url.query_pairs_mut()
        .append_pair("client_id", &client.id.to_string())
        .append_pair("name", &client.name)
        .append_pair("ice", &client.ice);
    if full {
        url.query_pairs_mut().append_pair("detail", "full");
    }
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| ApiError::transport("Client HTTP indisponible."))?;
    let mut response = http
        .get(url)
        .bearer_auth(token)
        .header("X-Atlas-Client", "native")
        .header("X-Device-Fingerprint", api.fingerprint())
        .send()
        .await
        .map_err(|_| ApiError::transport("Chargement du rapport impossible."))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(ApiError {
            status,
            code: None,
            message: format!("Rapport indisponible ({status})."),
        });
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("text/html")
    {
        return Err(ApiError::client(
            "Le serveur n'a pas renvoyé un rapport HTML.",
        ));
    }
    const MAX_BYTES: usize = 4 * 1024 * 1024;
    if response
        .content_length()
        .is_some_and(|n| n > MAX_BYTES as u64)
    {
        return Err(ApiError::client("Rapport trop volumineux."));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ApiError::transport("Lecture du rapport interrompue."))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
            return Err(ApiError::client("Rapport trop volumineux."));
        }
        bytes.extend_from_slice(&chunk);
    }
    let html = std::str::from_utf8(&bytes)
        .map_err(|_| ApiError::client("Encodage du rapport invalide."))?;
    parse_report(html)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_server_amounts_without_executing_or_including_scripts() {
        let html = r#"<div class="sheet"><div class="actions">Imprimer</div><div class="hdr"><h1>Situation client — Exemple</h1></div><div class="stat"><div>Reste dû</div><div>123 MAD</div></div><h2>Pièces</h2><table><tr><th>N°</th><th>Reste</th></tr><tr><td>FA-1</td><td>123<script>secret()</script></td></tr></table><h2>Reçus</h2><table><tr><td>Aucun reçu</td></tr></table><script>secret()</script></div>"#;
        let report = parse_report(html).unwrap();
        let dump = format!("{:?}", report.blocks);
        assert!(dump.contains("123 MAD"));
        assert!(dump.contains("FA-1"));
        assert!(!dump.contains("secret()"));
        assert!(!dump.contains("Imprimer"));
        assert!(parse_report("<html><h1>Connexion</h1></html>").is_err());
    }
}

//! Client situation from the worker's existing printable report.
//! Parse it into inert native text and tables: no scripts, external resources,
//! report tickets, session credentials in URLs, or recalculated balances.
use atlas_api::{ApiClient, ApiError};
use scraper::{ElementRef, Html, Selector};

#[derive(Clone, Debug)]
pub(crate) enum ReportBlock {
    Heading(String),
    Statistic { label: String, value: String },
    Text(String),
    Table(Vec<ReportRow>),
}
#[derive(Clone, Debug)]
pub(crate) struct ReportRow {
    pub header: bool,
    pub cells: Vec<String>,
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
        let rows: Vec<ReportRow> = element
            .select(row_sel)
            .map(|row| {
                let cells: Vec<_> = row.select(cell_sel).collect();
                ReportRow {
                    header: !cells.is_empty()
                        && cells.iter().all(|cell| cell.value().name() == "th"),
                    cells: cells.into_iter().map(text).collect(),
                }
            })
            .collect();
        blocks.push(ReportBlock::Table(rows));
    } else if element.value().classes().any(|c| c == "stat") {
        // These labels and values come straight from reportShell's server
        // output; never calculate or reinterpret client balances locally.
        let label = element
            .select(&Selector::parse(".l").expect("static selector"))
            .next()
            .map(text);
        let value = element
            .select(&Selector::parse(".v").expect("static selector"))
            .next()
            .map(text);
        if let (Some(label), Some(value)) = (label, value) {
            blocks.push(ReportBlock::Statistic { label, value });
        } else {
            let content = text(element);
            if !content.is_empty() {
                blocks.push(ReportBlock::Text(content));
            }
        }
    } else if matches!(tag, "h1" | "h2" | "h3") {
        let content = text(element);
        if !content.is_empty() {
            blocks.push(ReportBlock::Heading(content));
        }
    } else if matches!(tag, "p" | "th" | "td") {
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
            // Mixed text and inline elements occur in detail headers (e.g.
            // "TTC 100 · Payé 20 · <strong>Reste 80</strong>"). Keep
            // the text nodes in order rather than silently dropping totals.
            for child in element.children() {
                if let Some(child) = ElementRef::wrap(child) {
                    collect(child, blocks, row_sel, cell_sel);
                } else if let Some(node) = child.value().as_text() {
                    let content = node.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !content.is_empty() {
                        blocks.push(ReportBlock::Text(content));
                    }
                }
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
        let html = r#"<div class="sheet"><div class="actions">Imprimer</div><div class="hdr"><h1>Situation client — Exemple</h1></div><div class="stat"><div>Reste dû</div><div>123 MAD</div></div><h2>Pièces</h2><table><tr><th>N°</th><th>Reste</th></tr><tr><td>FA-1</td><td>123<script>secret()</script></td></tr></table><div>TTC 100 · Payé 20 · <strong>Reste 80</strong></div><h2>Reçus</h2><table><tr><td>Aucun reçu</td></tr></table><script>secret()</script></div>"#;
        let report = parse_report(html).unwrap();
        let dump = format!("{:?}", report.blocks);
        assert!(dump.contains("123 MAD"));
        assert!(dump.contains("FA-1"));
        assert!(dump.contains("TTC 100 · Payé 20"));
        assert!(dump.contains("Reste 80"));
        assert!(!dump.contains("secret()"));
        assert!(!dump.contains("Imprimer"));
        assert!(parse_report("<html><h1>Connexion</h1></html>").is_err());
    }
    #[test]
    fn retains_worker_summary_and_detailed_document_values() {
        // Mirrors reportShell + client-situation markup (including WR, VR,
        // an empty receipts row and the full-mode product table).
        let html = r#"<div class="sheet"><div class="hdr"><div><div class="co">Entreprise</div><h1>Situation client (détail) — Client Exemple</h1><p>Synthèse</p></div></div>
        <div class="grid"><div class="stat"><div class="l">Total facturé TTC</div><div class="v">1 200 MAD</div></div><div class="stat"><div class="l">Avoirs (WR)</div><div class="v">200 MAD</div></div><div class="stat"><div class="l">Déjà payé</div><div class="v">300 MAD</div></div><div class="stat"><div class="l">Reste dû</div><div class="v">900 MAD</div></div></div>
        <h2>Pièces (factures, tickets VR, avoirs WR)</h2><table><tr><th>N°</th><th>Reste</th></tr><tr><td>WR-1<div>Avoir / remboursement</div></td><td>0</td></tr><tr><td>VR-1<div>Vente rapide</div></td><td>900</td></tr></table>
        <h2>Détail produits par facture</h2><div>TTC 1200 · Payé 300 · <strong>Reste 900 MAD</strong></div><table><tr><th>Produit / service</th><th>Qté</th></tr><tr><td>Marbre <span>(SKU)</span></td><td>2</td></tr></table>
        <h2>Reçus d’encaissement</h2><table><tr><th>N°</th><th>Date</th></tr><tr><td colspan="2">Aucun reçu</td></tr></table></div>"#;
        let report = parse_report(html).unwrap();
        let stats: Vec<_> = report
            .blocks
            .iter()
            .filter_map(|block| match block {
                ReportBlock::Statistic { label, value } => Some((label.as_str(), value.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(
            stats,
            vec![
                ("Total facturé TTC", "1 200 MAD"),
                ("Avoirs (WR)", "200 MAD"),
                ("Déjà payé", "300 MAD"),
                ("Reste dû", "900 MAD")
            ]
        );
        let dump = format!("{:?}", report.blocks);
        for expected in [
            "WR-1",
            "VR-1",
            "Marbre",
            "SKU",
            "Reste 900 MAD",
            "Aucun reçu",
            "Détail produits par facture",
        ] {
            assert!(dump.contains(expected), "missing {expected}");
        }
    }
}

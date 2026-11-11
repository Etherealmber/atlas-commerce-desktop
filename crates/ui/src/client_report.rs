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
fn parse_document(
    html: &str,
    expected_title: &str,
    min_tables: usize,
) -> Result<ClientReport, ApiError> {
    let doc = Html::parse_document(html);
    let selector = |s: &str| Selector::parse(s).expect("static selector");
    let sheet = doc
        .select(&selector(".sheet"))
        .next()
        .ok_or_else(|| ApiError::client("Format du rapport non reconnu."))?;
    let title = sheet
        .select(&selector("h1"))
        .next()
        .map(text)
        .unwrap_or_default();
    if !title.starts_with(expected_title) || sheet.select(&selector("table")).count() < min_tables {
        return Err(ApiError::client("Format du rapport non reconnu."));
    }
    let mut blocks = Vec::new();
    collect(sheet, &mut blocks, &selector("tr"), &selector("th, td"));
    Ok(ClientReport { blocks })
}

fn parse_report(html: &str) -> Result<ClientReport, ApiError> {
    parse_document(html, "Situation client", 2)
}

#[cfg(feature = "visual-fixtures")]
pub(crate) fn fictional_receivables() -> ClientReport {
    parse_document(r#"<div class="sheet"><h1>Rapport créances clients</h1><div class="grid"><div class="stat"><div class="l">Total à recevoir</div><div class="v">4 750 MAD</div></div><div class="stat"><div class="l">Factures</div><div class="v">2</div></div></div><h2>Factures de vente impayées</h2><table><tr><th>N°</th><th>Client</th><th>Émission</th><th>Échéance</th><th>Statut</th><th>Reste dû</th></tr><tr><td>FA-DEMO-001</td><td>Client Exemple</td><td>2026-01-12</td><td>2026-02-12</td><td>partial</td><td>2 500 MAD</td></tr><tr><td>FA-DEMO-002</td><td>Société Démo</td><td>2026-01-20</td><td>—</td><td>sent</td><td>2 250 MAD</td></tr></table></div>"#, "Rapport créances clients", 1).expect("fixture is valid")
}

#[cfg(feature = "visual-fixtures")]
pub(crate) fn fictional_payables() -> ClientReport {
    parse_document(r#"<div class="sheet"><h1>Rapport dettes fournisseurs</h1><div class="grid"><div class="stat"><div class="l">Total à payer</div><div class="v">3 200 MAD</div></div><div class="stat"><div class="l">Factures d'achat</div><div class="v">1</div></div></div><h2>Factures d'achat impayées</h2><table><tr><th>N°</th><th>Fournisseur</th><th>Émission</th><th>Échéance</th><th>Statut</th><th>Reste dû</th></tr><tr><td>AF-DEMO-001</td><td>Fournisseur Exemple</td><td>2026-01-12</td><td>2026-02-12</td><td>partial</td><td>3 200 MAD</td></tr></table></div>"#, "Rapport dettes fournisseurs", 1).expect("fixture is valid")
}

/// The stock report can legitimately contain no product table. Require its
/// four summary labels so the warehouse chooser or a proxy page is not shown
/// as a valuation report. Values remain exactly as supplied by the worker.
fn parse_stock(html: &str) -> Result<ClientReport, ApiError> {
    let report = parse_document(html, "Rapport stock par catégories", 0)?;
    for expected in [
        "Valeur au coût",
        "Valeur à la vente",
        "Marge potentielle",
        "Alertes stock bas",
    ] {
        if !report
            .blocks
            .iter()
            .any(|block| matches!(block, ReportBlock::Statistic { label, .. } if label == expected))
        {
            return Err(ApiError::client("Format du rapport de stock non reconnu."));
        }
    }
    Ok(report)
}

/// A stock location from the worker's printable-report chooser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StockLocation {
    pub id: i64,
    pub label: String,
}

/// Extract only numeric location IDs and visible text. Hidden ticket fields,
/// scripts, links and form actions are never read or executed.
fn parse_stock_locations(html: &str) -> Result<Vec<StockLocation>, ApiError> {
    let doc = Html::parse_document(html);
    let selector = |s: &str| Selector::parse(s).expect("static selector");
    let sheet = doc
        .select(&selector(".sheet"))
        .next()
        .ok_or_else(|| ApiError::client("Liste des emplacements non reconnue."))?;
    let title = sheet
        .select(&selector("h1"))
        .next()
        .map(text)
        .unwrap_or_default();
    if !title.starts_with("Rapport stock — choisir l’entrepôt") {
        return Err(ApiError::client("Liste des emplacements non reconnue."));
    }
    let select = sheet
        .select(&selector("select[name=location_id]"))
        .next()
        .ok_or_else(|| ApiError::client("Liste des emplacements absente."))?;
    let mut locations = Vec::new();
    let mut has_all = false;
    for option in select.select(&selector("option")) {
        let value = option.value().attr("value").unwrap_or("");
        if value == "all" {
            has_all = true;
            continue;
        }
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let Some(id) = value.parse::<i64>().ok().filter(|id| *id > 0) else {
            continue;
        };
        let label: String = text(option).chars().take(160).collect();
        if !label.is_empty()
            && !locations
                .iter()
                .any(|location: &StockLocation| location.id == id)
        {
            locations.push(StockLocation { id, label });
            if locations.len() >= 1000 {
                return Err(ApiError::client("Liste des emplacements trop volumineuse."));
            }
        }
    }
    if !has_all {
        return Err(ApiError::client("Liste des emplacements non reconnue."));
    }
    Ok(locations)
}

/// Load the same active locations the official stock report offers.
pub(crate) async fn fetch_stock_locations(api: &ApiClient) -> Result<Vec<StockLocation>, ApiError> {
    let url = url::Url::parse(&format!("{}/api/reports/stock", api.base_url()))
        .map_err(|_| ApiError::client("Adresse du rapport invalide."))?;
    parse_stock_locations(&fetch_html(api, url).await?)
}

/// Validated query parameter for a report; None denotes all locations.
pub(crate) fn stock_location_param(location: Option<i64>) -> Result<String, ApiError> {
    match location {
        None => Ok("all".to_owned()),
        Some(id) if id > 0 => Ok(id.to_string()),
        Some(_) => Err(ApiError::client("Emplacement invalide.")),
    }
}

/// Ask the worker for the selected location; never calculate local totals.
pub(crate) async fn fetch_stock(
    api: &ApiClient,
    location: Option<i64>,
) -> Result<ClientReport, ApiError> {
    let location = stock_location_param(location)?;
    let mut url = url::Url::parse(&format!("{}/api/reports/stock", api.base_url()))
        .map_err(|_| ApiError::client("Adresse du rapport invalide."))?;
    url.query_pairs_mut().append_pair("location_id", &location);
    let html = fetch_html(api, url).await?;
    parse_stock(&html)
}

#[cfg(feature = "visual-fixtures")]
pub(crate) fn fictional_stock() -> ClientReport {
    parse_stock(r#"<div class="sheet"><h1>Rapport stock par catégories</h1><p>Entrepôt : Tous les emplacements</p><div class="grid"><div class="stat"><div class="l">Valeur au coût</div><div class="v">3 200 MAD</div></div><div class="stat"><div class="l">Valeur à la vente</div><div class="v">5 000 MAD</div></div><div class="stat"><div class="l">Marge potentielle</div><div class="v">1 800 MAD</div></div><div class="stat"><div class="l">Alertes stock bas</div><div class="v">1</div></div></div><h2>1. Marbre</h2><table><tr><th>Réf.</th><th>Produit</th><th>Qté</th><th>Coût unit.</th><th>Prix unit.</th><th>Valeur coût</th><th>Valeur vente</th><th>Marge</th></tr><tr><td>SKU-DÉMO</td><td>Pierre Exemple</td><td>10</td><td>320</td><td>500</td><td>3 200</td><td>5 000</td><td>1 800</td></tr></table></div>"#).expect("fixture is valid")
}

/// The server supplies the payable amounts and applies its own bill filters.
pub(crate) async fn fetch_payables(api: &ApiClient) -> Result<ClientReport, ApiError> {
    let url = url::Url::parse(&format!("{}/api/reports/payables", api.base_url()))
        .map_err(|_| ApiError::client("Adresse du rapport invalide."))?;
    let html = fetch_html(api, url).await?;
    parse_document(&html, "Rapport dettes fournisseurs", 1)
}

/// The server report is authoritative for the aggregate and the listed rows.
pub(crate) async fn fetch_receivables(api: &ApiClient) -> Result<ClientReport, ApiError> {
    let url = url::Url::parse(&format!("{}/api/reports/receivables", api.base_url()))
        .map_err(|_| ApiError::client("Adresse du rapport invalide."))?;
    let html = fetch_html(api, url).await?;
    parse_document(&html, "Rapport créances clients", 1)
}

pub(crate) async fn fetch(
    api: &ApiClient,
    client: &atlas_api::models::Client,
    full: bool,
) -> Result<ClientReport, ApiError> {
    if client.id <= 0 {
        return Err(ApiError::client("Identifiant client invalide."));
    }
    let mut url = url::Url::parse(&format!("{}/api/reports/client-situation", api.base_url()))
        .map_err(|_| ApiError::client("Adresse du rapport invalide."))?;
    url.query_pairs_mut()
        .append_pair("client_id", &client.id.to_string())
        .append_pair("name", &client.name)
        .append_pair("ice", &client.ice);
    if full {
        url.query_pairs_mut().append_pair("detail", "full");
    }
    let html = fetch_html(api, url).await?;
    parse_report(&html)
}

async fn fetch_html(api: &ApiClient, url: url::Url) -> Result<String, ApiError> {
    let token = api
        .token()
        .ok_or_else(|| ApiError::client("Session indisponible."))?;
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
    Ok(html.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_receivables_without_recalculating_or_accepting_other_pages() {
        let html = r#"<div class="sheet"><h1>Rapport créances clients</h1><div class="stat"><div class="l">Total à recevoir</div><div class="v">1 234 MAD</div></div><h2>Factures de vente impayées</h2><table><tr><th>N°</th><th>Reste dû</th></tr><tr><td>FA-1</td><td>1 234 MAD</td></tr></table><script>secret()</script></div>"#;
        let report = parse_document(html, "Rapport créances clients", 1).unwrap();
        let dump = format!("{:?}", report.blocks);
        assert!(dump.contains("1 234 MAD"));
        assert!(dump.contains("FA-1"));
        assert!(!dump.contains("secret()"));
        assert!(parse_report(html).is_err());
        assert!(
            parse_document(
                "<div class=\"sheet\"><h1>Rapport créances clients</h1></div>",
                "Rapport créances clients",
                1
            )
            .is_err()
        );
    }

    #[test]
    fn stock_location_chooser_uses_only_valid_options_and_ignores_secrets() {
        let html = r#"<div class="sheet"><h1>Rapport stock — choisir l’entrepôt</h1><form><input type="hidden" name="rt" value="secret-ticket"><select name="location_id"><option value="all">Tous</option><option value="42">Dépôt principal (défaut)</option><option value="42">Doublon</option><option value="-1">Négatif</option><option value="abc">Invalide</option><option value="7">Entrepôt &amp; magasin<script>secret()</script></option></select></form></div>"#;
        let locations = super::parse_stock_locations(html).unwrap();
        assert_eq!(
            locations,
            vec![
                super::StockLocation {
                    id: 42,
                    label: "Dépôt principal (défaut)".into()
                },
                super::StockLocation {
                    id: 7,
                    label: "Entrepôt & magasin".into()
                }
            ]
        );
        assert!(!format!("{locations:?}").contains("secret"));
        assert!(super::parse_stock_locations(r#"<div class="sheet"><h1>Rapport stock — choisir l’entrepôt</h1><select name="location_id"><option value="1">A</option></select></div>"#).is_err());
        assert!(super::parse_stock_locations("<h1>Connexion</h1>").is_err());
        assert_eq!(super::stock_location_param(None).unwrap(), "all");
        assert_eq!(super::stock_location_param(Some(42)).unwrap(), "42");
        assert!(super::stock_location_param(Some(0)).is_err());
        assert!(super::stock_location_param(Some(-1)).is_err());
    }

    #[test]
    fn stock_report_accepts_empty_inventory_but_rejects_a_chooser() {
        let body = r#"<div class="sheet"><h1>Rapport stock par catégories</h1><div class="stat"><div class="l">Valeur au coût</div><div class="v">0 MAD</div></div><div class="stat"><div class="l">Valeur à la vente</div><div class="v">0 MAD</div></div><div class="stat"><div class="l">Marge potentielle</div><div class="v">0 MAD</div></div><div class="stat"><div class="l">Alertes stock bas</div><div class="v">0</div></div><p>Aucun produit actif sur cet emplacement.</p><script>secret()</script></div>"#;
        let report = super::parse_stock(body).unwrap();
        let dump = format!("{:?}", report.blocks);
        assert!(dump.contains("Aucun produit actif"));
        assert!(dump.contains("0 MAD"));
        assert!(!dump.contains("secret()"));
        assert!(
            super::parse_stock(
                r#"<div class="sheet"><h1>Rapport stock — choisir l’entrepôt</h1></div>"#
            )
            .is_err()
        );
        assert!(
            super::parse_stock(r#"<div class="sheet"><h1>Rapport stock par catégories</h1></div>"#)
                .is_err()
        );
    }

    #[test]
    fn stock_report_preserves_server_rows_and_values() {
        let body = r#"<div class="sheet"><h1>Rapport stock par catégories</h1><div class="stat"><div class="l">Valeur au coût</div><div class="v">99 MAD</div></div><div class="stat"><div class="l">Valeur à la vente</div><div class="v">101 MAD</div></div><div class="stat"><div class="l">Marge potentielle</div><div class="v">2 MAD</div></div><div class="stat"><div class="l">Alertes stock bas</div><div class="v">1</div></div><h2>Marbre</h2><table><tr><th>Produit</th><th>Qté</th></tr><tr><td>Exemple</td><td>7</td></tr></table></div>"#;
        let report = super::parse_stock(body).unwrap();
        let dump = format!("{:?}", report.blocks);
        for value in ["99 MAD", "101 MAD", "2 MAD", "Exemple", "7"] {
            assert!(dump.contains(value), "missing {value}");
        }
    }

    #[test]
    fn payables_use_the_server_amount_and_reject_other_report_types() {
        let html = r#"<div class="sheet"><h1>Rapport dettes fournisseurs</h1><div class="stat"><div class="l">Total à payer</div><div class="v">3 200 MAD</div></div><table><tr><th>N°</th><th>Reste dû</th></tr><tr><td>AF-1</td><td>3 200 MAD</td></tr></table><script>do_not_run()</script></div>"#;
        let report = parse_document(html, "Rapport dettes fournisseurs", 1).unwrap();
        let dump = format!("{:?}", report.blocks);
        assert!(dump.contains("3 200 MAD"));
        assert!(dump.contains("AF-1"));
        assert!(!dump.contains("do_not_run"));
        assert!(parse_document(html, "Rapport créances clients", 1).is_err());
        assert!(
            parse_document(
                "<div class=\"sheet\"><h1>Rapport dettes fournisseurs</h1></div>",
                "Rapport dettes fournisseurs",
                1
            )
            .is_err()
        );
    }

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

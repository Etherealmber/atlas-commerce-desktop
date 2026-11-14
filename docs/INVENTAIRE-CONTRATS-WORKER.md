# Inventaire de travail des contrats Worker

Référence : `Etherealmber/atlas-commerce`, commit `e00f186f68204ebeee421943c73f5e6f6d9c89fb`. Cet inventaire **n’est pas un audit complet des schémas** : les déclarations statiques ci-dessous sont un index de navigation, pas une preuve de montage ou d’autorisation. Les routes calculées, les méthodes enregistrées autrement, les conditions et les schémas doivent être vérifiés dans le code avant usage. Les fichiers et numéros de ligne correspondent au commit cité.

## Architecture et frontières

Le serveur est un Worker Hono ; `worker/index.ts` enregistre aussi les modules de `worker/routes/`. La session Atlas existante est transmise par `Authorization: Bearer <token>` et liée à l’empreinte et à l’IP. Le client desktop existant utilise `X-Device-Fingerprint`, `X-Atlas-Client: native` et `Accept: application/json` ; les réponses peuvent différer entre enveloppes et authentification. Le health public `GET /api/health` n’authentifie pas la session. L’API partenaire `GET /api/v1/health` utilise une clé `X-Api-Key` et ne remplace pas le health desktop. Les montants sont en unités majeures (DH), non en centimes. Finance et paie sont calculées côté serveur et soumises aux rôles ; ne pas recalculer ou compter deux fois leurs écritures miroir.

## Authentification examinée

| Méthode et route | Contrat vérifié |
| --- | --- |
| `GET /api/health` | Santé publique ; ne valide ni compte ni droits. |
| `POST /api/auth/login` | JSON `username`, `password`, `fingerprint`, `client` ; réponses session ou OTP, éventuel refus Turnstile et quotas. Voir `worker/index.ts` et `docs/login-rate-limits.md`. |
| `POST /api/auth/verify-otp` | Vérification du code dans le contexte de connexion ; mêmes budgets OTP que le lien. |
| `POST /api/auth/verify-link` | Vérification du lien signé ; ce mécanisme est distinct du protocole desktop v1. |
| `GET /api/auth/me` | Profil de la session bearer. |
| `GET /api/auth/desktop/v1/capabilities` | Découverte de la version et du flux supporté. |
| `POST /api/auth/desktop/v1/requests` | JSON `origin`, `state`, `code_challenge`, `code_challenge_method: S256`, `device_label` ; `X-Device-Fingerprint`. Retourne ID, URL d’autorisation, code de comparaison, expiration et intervalle. |
| `POST /api/auth/desktop/v1/prepare`, `/authorize` | Consentement dans la session web, bearer et empreinte navigateur ; `prepare` produit une nonce CSRF utilisée par `authorize` avec `decision`. |
| `POST /api/auth/desktop/v1/poll`, `/exchange`, `/cancel` | JSON `request_id`, `origin`, `state`, `code_verifier` et empreinte desktop. Poll sans identité ni jeton ; échange unique retourne le bearer ; annulation idempotente avant consommation. Pas de callback. |
| `GET /api/auth/webauthn/status` | Capacité `data.enabled` ; les quatre routes options/verify retournent `WEBAUTHN_DISABLED` si désactivées. |
| `GET /api/auth/devices`, `POST /api/auth/devices/revoke` | Appareils de confiance ; distincts des routes `/api/auth/trusted-devices`. |

Les requêtes v1 exigent HTTPS/origine exacte, PKCE S256, état, empreinte/IP, consentement et expiration. Échanges au maximum 15 s, intervalle minimum de polling 5 s, durée 300 s ; le contrat ne transporte jamais le bearer dans une URL ou un callback. Après échange, l’annulation ne révoque pas la session.

## Migrations

- `0001` : journal d’audit, images produit par URL et convention monétaire en unités majeures.
- `0002` et `0003` : index de filtres et de relations, sans nouveaux workflows.
- `0004` : coût d’achat unitaire précédent (`last_purchase_cost`).
- `0005` : paramètres de confidentialité QR et rétention client.
- `0006` : indicateur de situation client sur les reçus.
- `0007` : index liste produit et recherche code-barres.
- `0008` : marqueur sans opération ; ajout des colonnes de profil assuré au runtime.

## Correspondance desktop : point de départ, non validation exhaustive

Les méthodes ci-dessous existent dans `crates/api` ; chaque mutation et ses droits doivent encore être confrontés au handler avant extension. Les paramètres, réponses et tris ne sont pas interchangeables entre listes et rapports.

| Domaine | Appels desktop existants |
| --- | --- |
| Tableau de bord | `GET /api/dashboard`, `/api/alerts`, `/api/nav-badges` |
| Clients | `GET/POST /api/clients`, `PUT /api/clients/:id` |
| Produits et stock | `GET/POST /api/products`, `GET /api/products/:id`, `POST /api/products/:id/adjust-stock`, `GET /api/scan/barcode/:code` |
| Catégories et unités | `GET/POST /api/categories`, `PUT /api/categories/:id`, `GET/POST /api/units`, `PUT /api/units/:id` |
| Factures et ventes | `GET/POST /api/invoices` ; le filtre `kind` et la classification des documents doivent rester conformes au serveur. |
| Dépenses | `GET/POST /api/expenses` ; migrations reçus et situations ne prouvent pas à elles seules une API de pièces jointes. |
| Livraison | `GET/POST /api/delivery-notes` |
| Fournisseurs | `GET/POST /api/vendors`, `PUT /api/vendors/:id` |
| Agent | `GET /api/agent/policy`, `POST /api/assist/chat` |
| Documents | `GET /api/reports/link-ticket` ; les liens de rapport utilisent un ticket signé, pas le bearer dans l’URL. |

## Risques techniques à instruire

- Les jobs HTTP sont lancés sur un runtime Tokio et consultés par canal ; `Job::poll` utilise néanmoins un mutex et ne distingue pas un calcul encore actif d’un canal déconnecté. Un contrôle complet des chemins d’erreur et d’annulation reste nécessaire.
- Le lanceur `open::that` est appelé depuis des chemins UI de repli navigateur et peut bloquer selon le système. Ce constat n’établit pas à lui seul la cause d’un incident utilisateur particulier.
- Le client HTTP général applique actuellement un timeout de 20 s ; l’authentification navigateur v1 et le health ont leurs propres limites. Leurs comportements en cas d’expiration et de restauration de session nécessitent des tests ciblés supplémentaires.
- Aucun changement de serveur installé ni formulaire de mot de passe natif ne doit contourner les exigences de verrouillage et de connexion hébergée précédemment fixées. Les effets Mica/Acrylic translucides demandés sont en conflit avec la contrainte antérieure de surfaces opaques.

## Index mécanique des déclarations de routes

Index généré par recherche des appels littéraux Hono dans `worker/index.ts` et `worker/routes/*.ts`, hors tests. `DESKTOP_AUTH` vaut `/api/auth/desktop/v1`. Une déclaration peut être non montée, conditionnelle ou n’être qu’un alias ; examiner le middleware, le corps, les champs et les réponses dans la source avant d’implémenter un écran.

| Source | Méthode | Expression de chemin |
| --- | --- | --- |
| `worker/index.ts:1261` | GET | `/api/admin/reliability` |
| `worker/index.ts:1270` | POST | `/api/admin/search-index/rebuild` |
| `worker/index.ts:1480` | GET | `/api/agent/policy` |
| `worker/index.ts:1503` | POST | `/api/agent/usage` |
| `worker/index.ts:1515` | GET | `/api/admin/agent-usage-log` |
| `worker/index.ts:1528` | GET | `/api/canary` |
| `worker/index.ts:1580` | GET | `/api/admin/db-migrate/status` |
| `worker/index.ts:1594` | POST | `/api/admin/db-migrate/copy` |
| `worker/index.ts:1641` | POST | `/api/admin/db-migrate/cutover` |
| `worker/index.ts:1660` | POST | `/api/agent/flag` |
| `worker/index.ts:1677` | GET | `/api/agent/knowledge` |
| `worker/index.ts:1703` | POST | `/api/agent/knowledge/memory` |
| `worker/index.ts:1720` | DELETE | `/api/agent/knowledge/memory/:id` |
| `worker/index.ts:1733` | PUT | `/api/agent/knowledge/context` |
| `worker/index.ts:1741` | POST | `/api/agent/knowledge/guardrails` |
| `worker/index.ts:1753` | PUT | `/api/agent/knowledge/guardrails/:id` |
| `worker/index.ts:1762` | DELETE | `/api/agent/knowledge/guardrails/:id` |
| `worker/index.ts:1770` | PUT | `/api/agent/knowledge/tools` |
| `worker/index.ts:1780` | POST | `/api/agent/knowledge/docs` |
| `worker/index.ts:1792` | DELETE | `/api/agent/knowledge/docs/:id` |
| `worker/index.ts:1800` | GET | `/api/admin/agent-controls` |
| `worker/index.ts:1808` | POST | `/api/admin/agent-access` |
| `worker/index.ts:1822` | POST | `/api/settings/agent-data/request-delete-otp` |
| `worker/index.ts:1831` | POST | `/api/settings/agent-data/delete` |
| `worker/index.ts:1855` | POST | `/api/settings/account/request-delete-otp` |
| `worker/index.ts:1864` | POST | `/api/settings/account/delete` |
| `worker/index.ts:1881` | GET | `/api/settings/2fa` |
| `worker/index.ts:1888` | PUT | `/api/settings/2fa` |
| `worker/index.ts:1940` | GET | `/api/health` |
| `worker/index.ts:1970` | GET | `/api/admin/r2/images` |
| `worker/index.ts:2716` | GET | `/api/auth/turnstile-config` |
| `worker/index.ts:2735` | POST | `/api/auth/login` |
| `worker/index.ts:2980` | POST | `/api/auth/verify-otp` |
| `worker/index.ts:3112` | POST | `/api/auth/verify-link` |
| `worker/index.ts:3248` | POST | `/api/auth/qr/pc-session` |
| `worker/index.ts:3383` | POST | `/api/auth/qr/session/:token/phone-bind` |
| `worker/index.ts:3442` | POST | `/api/auth/qr/session` |
| `worker/index.ts:3514` | GET | `/api/auth/qr/session/:token` |
| `worker/index.ts:3595` | POST | `/api/auth/qr/session/:token/claim` |
| `worker/index.ts:3643` | POST | `/api/auth/qr/session/:token/approve` |
| `worker/index.ts:3676` | POST | `/api/auth/qr/session/:token/deny` |
| `worker/index.ts:3694` | POST | `/api/auth/qr/session/:token/complete` |
| `worker/index.ts:3849` | POST | `/api/auth/webauthn/register/options` |
| `worker/index.ts:3896` | POST | `/api/auth/webauthn/register/verify` |
| `worker/index.ts:3990` | POST | `/api/auth/webauthn/login/options` |
| `worker/index.ts:4022` | POST | `/api/auth/webauthn/login/verify` |
| `worker/index.ts:4141` | GET | `/api/auth/webauthn/status` |
| `worker/index.ts:4172` | GET | `/.well-known/assetlinks.json` |
| `worker/index.ts:4204` | POST | `/api/crash-reports` |
| `worker/index.ts:4220` | POST | `/api/reports` |
| `worker/index.ts:4255` | GET | `/api/admin/github-token` |
| `worker/index.ts:4265` | PUT | `/api/admin/github-token` |
| `worker/index.ts:4288` | GET | `/api/admin/reports` |
| `worker/index.ts:4300` | POST | `/api/admin/reports/retry` |
| `worker/index.ts:4313` | GET | `/api/auth/webauthn/credentials` |
| `worker/index.ts:4391` | PATCH | `/api/auth/webauthn/credentials/:id` |
| `worker/index.ts:4497` | DELETE | `/api/auth/webauthn/credentials/:id` |
| `worker/index.ts:4522` | POST | `/api/auth/webauthn/credentials/delete` |
| `worker/index.ts:4549` | POST | `/api/auth/register-biometric` |
| `worker/index.ts:4558` | POST | `/api/auth/login-biometric` |
| `worker/index.ts:4569` | GET | `/api/auth/trusted-devices` |
| `worker/index.ts:4614` | DELETE | `/api/auth/trusted-devices/:fingerprint` |
| `worker/index.ts:4635` | POST | `/api/auth/device-status` |
| `worker/index.ts:4725` | POST | `/api/admin/users` |
| `worker/index.ts:4774` | DELETE | `/api/admin/users/:id` |
| `worker/index.ts:4790` | PUT | `/api/admin/users/:id` |
| `worker/index.ts:4832` | GET | `/api/admin/audit` |
| `worker/index.ts:4942` | GET | `/api/admin/audit/export` |
| `worker/index.ts:4988` | GET | `/api/admin/agent-permissions` |
| `worker/index.ts:5010` | PUT | `/api/admin/agent-permissions` |
| `worker/index.ts:5036` | GET | `/api/admin/config` |
| `worker/index.ts:5082` | PUT | `/api/admin/config` |
| `worker/index.ts:5110` | GET | `/api/me/avatar` |
| `worker/index.ts:5187` | POST | `/api/auth/logout` |
| `worker/index.ts:5213` | GET | `/api/auth/me` |
| `worker/index.ts:5295` | PATCH | `/api/auth/me` |
| `worker/index.ts:5447` | POST | `/api/admin/retention/run` |
| `worker/index.ts:5538` | GET | `/api/admin/r2/catalog` |
| `worker/index.ts:5546` | GET | `/api/public/products` |
| `worker/index.ts:5550` | GET | `/api/public/products/:id` |
| `worker/index.ts:5554` | GET | `/api/public/categories` |
| `worker/index.ts:5560` | POST | `/api/public/invoices/:token/sign` |
| `worker/index.ts:5616` | GET | `/api/public/invoices/:token/signature` |
| `worker/index.ts:5651` | GET | `/api/public/invoices/:token/timestamp` |
| `worker/index.ts:5678` | GET | `/api/public/invoices/:token` |
| `worker/index.ts:5875` | POST | `/api/admin/revoke-public-token` |
| `worker/index.ts:5892` | PATCH | `/api/receipts/:id/client-situation` |
| `worker/index.ts:5911` | GET | `/api/public/qr.svg` |
| `worker/index.ts:5927` | GET | `/api/public/verify` |
| `worker/index.ts:6049` | GET | `/api/public/company` |
| `worker/index.ts:6058` | GET | `/api/public/client-status` |
| `worker/index.ts:6315` | GET | `/api/settings` |
| `worker/index.ts:6340` | POST | `/api/admin/rotate-hmac-keys` |
| `worker/index.ts:6357` | GET | `/api/admin/hmac-status` |
| `worker/index.ts:6378` | PUT | `/api/settings` |
| `worker/index.ts:6531` | GET | `/api/categories` |
| `worker/index.ts:6546` | POST | `/api/categories` |
| `worker/index.ts:6565` | PUT | `/api/categories/:id` |
| `worker/index.ts:6585` | DELETE | `/api/categories/:id` |
| `worker/index.ts:6623` | GET | `/api/units` |
| `worker/index.ts:6638` | POST | `/api/units` |
| `worker/index.ts:6657` | PUT | `/api/units/:id` |
| `worker/index.ts:6677` | DELETE | `/api/units/:id` |
| `worker/index.ts:6742` | GET | `/api/products` |
| `worker/index.ts:6853` | GET | `/api/products/:id` |
| `worker/index.ts:6873` | POST | `/api/products` |
| `worker/index.ts:6971` | PUT | `/api/products/:id` |
| `worker/index.ts:7095` | POST | `/api/products/:id/adjust-stock` |
| `worker/index.ts:7155` | GET | `/api/products/:id/variants` |
| `worker/index.ts:7166` | POST | `/api/products/:id/variants` |
| `worker/index.ts:7248` | PUT | `/api/products/variants/:vid` |
| `worker/index.ts:7297` | DELETE | `/api/products/variants/:vid` |
| `worker/index.ts:7326` | GET | `/api/scan/barcode/:code` |
| `worker/index.ts:7438` | POST | `/api/scan/quick-product` |
| `worker/index.ts:7603` | POST | `/api/products/:id/request-delete-otp` |
| `worker/index.ts:7612` | DELETE | `/api/products/:id` |
| `worker/index.ts:7675` | POST | `/api/products/request-bulk-delete-otp` |
| `worker/index.ts:7688` | POST | `/api/products/bulk-delete` |
| `worker/index.ts:7743` | GET | `/api/catalog/settings` |
| `worker/index.ts:7749` | PUT | `/api/catalog/settings` |
| `worker/index.ts:7768` | GET | `/api/catalog/products` |
| `worker/index.ts:7784` | PUT | `/api/catalog/products/:id` |
| `worker/index.ts:7849` | GET | `/api/media/proxy` |
| `worker/index.ts:7926` | GET | `/api/catalog/products/:id/images` |
| `worker/index.ts:7935` | POST | `/api/catalog/products/:id/images` |
| `worker/index.ts:7988` | PUT | `/api/catalog/products/:id/images/:imageId` |
| `worker/index.ts:8026` | DELETE | `/api/catalog/products/:id/images/:imageId` |
| `worker/index.ts:8055` | GET | `/api/public/catalog` |
| `worker/index.ts:8090` | GET | `/api/vendors` |
| `worker/index.ts:8113` | POST | `/api/vendors` |
| `worker/index.ts:8125` | PUT | `/api/vendors/:id` |
| `worker/index.ts:8144` | DELETE | `/api/vendors/:id` |
| `worker/index.ts:9590` | POST | `/api/invoices` |
| `worker/index.ts:9895` | POST | `/api/invoices/remboursement` |
| `worker/index.ts:10346` | PATCH | `/api/invoices/:id/status` |
| `worker/index.ts:10412` | PUT | `/api/invoices/:id` |
| `worker/index.ts:10598` | POST | `/api/invoices/:id/payments` |
| `worker/index.ts:10770` | GET | `/api/invoices/:id/payments` |
| `worker/index.ts:10780` | POST | `/api/invoices/:id/convert-quote` |
| `worker/index.ts:10860` | GET | `/api/invoices/:id/print` |
| `worker/index.ts:11127` | POST | `/api/invoices/:id/request-delete-otp` |
| `worker/index.ts:11160` | GET | `/api/invoices/:id/links` |
| `worker/index.ts:11178` | DELETE | `/api/invoices/:id` |
| `worker/index.ts:11343` | GET | `/api/invoices/:id/pdf` |
| `worker/index.ts:11404` | GET | `/api/print/thermal/:id` |
| `worker/index.ts:11485` | GET | `/api/invoices/:id/ubl` |
| `worker/index.ts:11522` | GET | `/api/invoices/:id/ubl/validate` |
| `worker/index.ts:11557` | POST | `/api/invoices/:id/ubl/prepare` |
| `worker/index.ts:11626` | GET | `/api/invoices/:id/ubl/clearance` |
| `worker/index.ts:11672` | POST | `/api/invoices/:id/ubl/status` |
| `worker/index.ts:11728` | GET | `/api/invoices/:id/ubl/snapshot` |
| `worker/index.ts:11749` | GET | `/api/receipts/:id/pdf` |
| `worker/index.ts:11785` | GET | `/api/bills` |
| `worker/index.ts:11795` | GET | `/api/bills/:id` |
| `worker/index.ts:11830` | POST | `/api/bills/parse-document` |
| `worker/index.ts:11906` | POST | `/api/bills` |
| `worker/index.ts:11956` | PATCH | `/api/bills/:id/status` |
| `worker/index.ts:12036` | POST | `/api/bills/:id/payments` |
| `worker/index.ts:12141` | GET | `/api/bills/:id/payments` |
| `worker/index.ts:12150` | GET | `/api/bills/:id/print` |
| `worker/index.ts:12160` | POST | `/api/bills/:id/request-delete-otp` |
| `worker/index.ts:12169` | DELETE | `/api/bills/:id` |
| `worker/index.ts:12214` | POST | `/api/bills/request-bulk-delete-otp` |
| `worker/index.ts:12227` | POST | `/api/bills/bulk-delete` |
| `worker/index.ts:12240` | GET | `/api/receipts` |
| `worker/index.ts:12259` | POST | `/api/receipts` |
| `worker/index.ts:12331` | PUT | `/api/receipts/:id` |
| `worker/index.ts:12352` | GET | `/api/receipts/:id/print` |
| `worker/index.ts:12437` | POST | `/api/receipts/:id/request-delete-otp` |
| `worker/index.ts:12470` | GET | `/api/receipts/:id/links` |
| `worker/index.ts:12561` | DELETE | `/api/invoices/:id/payments/:paymentId` |
| `worker/index.ts:12606` | DELETE | `/api/receipts/:id` |
| `worker/index.ts:12666` | GET | `/api/expenses` |
| `worker/index.ts:12819` | GET | `/api/expenses/:id/links` |
| `worker/index.ts:12850` | DELETE | `/api/expenses/:id` |
| `worker/index.ts:12900` | GET | `/api/ledger/reconcile` |
| `worker/index.ts:12965` | POST | `/api/ledger/fix-receipts` |
| `worker/index.ts:13003` | GET | `/api/integrity/status` |
| `worker/index.ts:13050` | POST | `/api/integrity/regression` |
| `worker/index.ts:13163` | GET | `/api/fiscal/period-lock` |
| `worker/index.ts:13169` | PUT | `/api/fiscal/period-lock` |
| `worker/index.ts:13247` | GET | `/api/reports/link-ticket` |
| `worker/index.ts:13257` | GET | `/api/reports/aging` |
| `worker/index.ts:13284` | GET | `/api/reports/aging.csv` |
| `worker/index.ts:13322` | GET | `/api/reports/tva` |
| `worker/index.ts:13408` | GET | `/api/reports/tva.csv` |
| `worker/index.ts:13465` | GET | `/api/reports/aging.html` |
| `worker/index.ts:13518` | GET | `/api/reports/tva.html` |
| `worker/index.ts:13602` | GET | `/api/inventory/reorder` |
| `worker/index.ts:13621` | POST | `/api/inventory/reorder/draft-bill` |
| `worker/index.ts:13973` | GET | `/api/inventory/locations` |
| `worker/index.ts:14000` | POST | `/api/inventory/locations` |
| `worker/index.ts:14017` | PATCH | `/api/inventory/locations/:id` |
| `worker/index.ts:14037` | DELETE | `/api/inventory/locations/:id` |
| `worker/index.ts:14083` | GET | `/api/inventory/products/:id/placements` |
| `worker/index.ts:14128` | GET | `/api/inventory/stock-report` |
| `worker/index.ts:14257` | POST | `/api/inventory/transfer` |
| `worker/index.ts:14290` | POST | `/api/inventory/cycle-count` |
| `worker/index.ts:14319` | GET | `/api/inventory/valuation` |
| `worker/index.ts:14377` | GET | `/api/inventory/auto-transfer-rules` |
| `worker/index.ts:14383` | POST | `/api/inventory/auto-transfer-rules` |
| `worker/index.ts:14409` | DELETE | `/api/inventory/auto-transfer-rules/:id` |
| `worker/index.ts:14416` | POST | `/api/inventory/auto-transfer-rules/run` |
| `worker/index.ts:14472` | GET | `/api/integrations/keys` |
| `worker/index.ts:14482` | POST | `/api/integrations/keys` |
| `worker/index.ts:14504` | DELETE | `/api/integrations/keys/:id` |
| `worker/index.ts:14511` | GET | `/api/integrations/webhooks` |
| `worker/index.ts:14519` | POST | `/api/integrations/webhooks` |
| `worker/index.ts:14537` | DELETE | `/api/integrations/webhooks/:id` |
| `worker/index.ts:14545` | GET | `/api/reports/cogs` |
| `worker/index.ts:14616` | GET | `/api/delivery-notes` |
| `worker/index.ts:14622` | POST | `/api/delivery-notes` |
| `worker/index.ts:14660` | GET | `/api/delivery-notes/:id/print` |
| `worker/index.ts:14688` | POST | `/api/bills/:id/receive` |
| `worker/index.ts:14734` | GET | `/api/bills/:id/match` |
| `worker/index.ts:14763` | POST | `/api/bank/reconcile-csv` |
| `worker/index.ts:14973` | GET | `/api/nav-badges` |
| `worker/index.ts:15064` | GET | `/api/dashboard` |
| `worker/index.ts:15404` | POST | `/api/invoices/:id/send-email` |
| `worker/index.ts:15609` | GET | `/api/reports/balance` |
| `worker/index.ts:15674` | GET | `/api/reports/stock` |
| `worker/index.ts:15850` | GET | `/api/reports/receivables` |
| `worker/index.ts:15886` | GET | `/api/reports/payables` |
| `worker/index.ts:16109` | GET | `/api/assist/transcripts` |
| `worker/index.ts:16137` | GET | `/api/assist/transcripts/get` |
| `worker/index.ts:16214` | POST | `/api/assist/chat` |
| `worker/index.ts:16363` | POST | `/api/assist/chat/stream` |
| `worker/index.ts:16499` | GET | `/api/assist/next-actions` |
| `worker/index.ts:16528` | POST | `/api/assist/approve` |
| `worker/index.ts:16609` | POST | `/api/push/register` |
| `worker/index.ts:16638` | GET | `/api/assist/status` |
| `worker/index.ts:16660` | GET | `/api/settings/assist-provider` |
| `worker/index.ts:16675` | PUT | `/api/settings/assist-provider` |
| `worker/index.ts:16814` | GET | `/api/admin/app-update` |
| `worker/index.ts:16842` | PUT | `/api/admin/app-update` |
| `worker/index.ts:16915` | POST | `/api/admin/app-update/apk` |
| `worker/index.ts:16997` | GET | `/api/alerts` |
| `worker/index.ts:17037` | GET | `/api/stock/product-requests` |
| `worker/index.ts:17048` | POST | `/api/stock/product-requests` |
| `worker/index.ts:17083` | POST | `/api/stock/product-requests/:id/close` |
| `worker/index.ts:17633` | POST | `/api/admin/backup-now` |
| `worker/index.ts:17647` | GET | `/api/admin/backup-status` |
| `worker/index.ts:17665` | GET | `/api/admin/backups` |
| `worker/index.ts:17708` | GET | `/api/admin/backups/download` |
| `worker/index.ts:17785` | GET | `/api/admin/backups/restore-status` |
| `worker/index.ts:17810` | POST | `/api/admin/backups/restore-cancel` |
| `worker/index.ts:17855` | POST | `/api/admin/backups/restore` |
| `worker/index.ts:18102` | POST | `/api/admin/backups/purge-expired` |
| `worker/index.ts:18199` | POST | `/api/import/validate` |
| `worker/index.ts:18287` | POST | `/api/import/commit` |
| `worker/index.ts:18377` | GET | `/api/reports/client-situation` |
| `worker/index.ts:18599` | GET | `/api/reports/employee-blank-form` |
| `worker/index.ts:18605` | GET | `/api/reports/employee-situation` |
| `worker/index.ts:18647` | GET | `/api/reports/employees` |
| `worker/index.ts:18692` | GET | `/api/reports/combined` |
| `worker/index.ts:18932` | GET | `/api/attendance` |
| `worker/index.ts:19109` | POST | `/api/attendance/check-in` |
| `worker/index.ts:19129` | POST | `/api/attendance/check-out` |
| `worker/index.ts:19156` | PUT | `/api/attendance` |
| `worker/index.ts:19186` | POST | `/api/attendance/mark-absent-missing` |
| `worker/index.ts:19211` | GET | `/api/attendance/record/:employeeId` |
| `worker/index.ts:19343` | GET | `/api/attendance/weekly` |
| `worker/index.ts:19390` | GET | `/api/attendance/export` |
| `worker/index.ts:19438` | POST | `/api/attendance/self-check` |
| `worker/index.ts:19476` | POST | `/api/bills/import` |
| `worker/index.ts:19554` | GET | `/api/export/invoices` |
| `worker/index.ts:19580` | GET | `/api/export/products` |
| `worker/index.ts:19663` | GET | `/api/export/expenses` |
| `worker/index.ts:19683` | GET | `/api/export/clients` |
| `worker/index.ts:19695` | GET | `/api/export/payments` |
| `worker/index.ts:19719` | GET | `/api/export/month-pack` |
| `worker/index.ts:19884` | GET | `/api/mobile-actions` |
| `worker/index.ts:19927` | GET | `/api/mobile-actions/stats` |
| `worker/index.ts:19960` | GET | `/api/mobile-actions/:id` |
| `worker/index.ts:19970` | POST | `/api/mobile-actions` |
| `worker/index.ts:20036` | POST | `/api/mobile-actions/:id/complete` |
| `worker/index.ts:20129` | POST | `/api/mobile-actions/:id/reject` |
| `worker/index.ts:20168` | POST | `/api/inventory/cycle-count/batch` |
| `worker/index.ts:20224` | GET | `/api/hub/snapshot` |
| `worker/index.ts:20263` | GET | `/.well-known/assetlinks.json` |
| `worker/index.ts:20783` | GET | `/api/employees/form/print` |
| `worker/index.ts:20788` | GET | `/api/employees/:id/form/print` |
| `worker/index.ts:21355` | POST | `/api/cron/backup-execute` |
| `worker/index.ts:21426` | GET | `*` |
| `worker/routes/admin.ts:14` | GET | `/api/admin/users` |
| `worker/routes/androidRelease.ts:69` | POST | `/api/admin/android-release` |
| `worker/routes/androidRelease.ts:266` | GET | `/api/app/android-update` |
| `worker/routes/atlas-v209.ts:145` | POST | `/api/stock/receive` |
| `worker/routes/atlas-v209.ts:169` | POST | `/api/stock/count` |
| `worker/routes/atlas-v209.ts:188` | GET | `/api/stock/lots` |
| `worker/routes/atlas-v209.ts:198` | POST | `/api/products/:id/location` |
| `worker/routes/atlas-v209.ts:207` | GET | `/api/vendors/parts` |
| `worker/routes/atlas-v209.ts:215` | POST | `/api/vendors/parts` |
| `worker/routes/atlas-v209.ts:225` | POST | `/api/invoices/:id/pipeline` |
| `worker/routes/atlas-v209.ts:241` | GET | `/api/invoices/:id/pipeline` |
| `worker/routes/atlas-v209.ts:249` | GET | `/api/kits` |
| `worker/routes/atlas-v209.ts:258` | POST | `/api/kits` |
| `worker/routes/atlas-v209.ts:270` | POST | `/api/kits/:id/assemble` |
| `worker/routes/atlas-v209.ts:294` | GET | `/api/webhooks` |
| `worker/routes/atlas-v209.ts:300` | POST | `/api/webhooks` |
| `worker/routes/atlas-v209.ts:308` | DELETE | `/api/webhooks/:id` |
| `worker/routes/atlas-v209.ts:316` | GET | `/api/atlas/alerts` |
| `worker/routes/atlas-v209.ts:322` | POST | `/api/atlas/alerts` |
| `worker/routes/atlas-v209.ts:338` | GET | `/api/products/:id/label.svg` |
| `worker/routes/clients.ts:51` | GET | `/api/clients` |
| `worker/routes/clients.ts:77` | POST | `/api/clients` |
| `worker/routes/clients.ts:103` | PUT | `/api/clients/:id` |
| `worker/routes/clients.ts:132` | POST | `/api/clients/:id/request-delete-otp` |
| `worker/routes/clients.ts:143` | DELETE | `/api/clients/:id` |
| `worker/routes/clients.ts:171` | POST | `/api/clients/request-bulk-delete-otp` |
| `worker/routes/clients.ts:186` | POST | `/api/clients/bulk-delete` |
| `worker/routes/clients.ts:203` | POST | `/api/clients/import/preview` |
| `worker/routes/clients.ts:278` | POST | `/api/clients/import` |
| `worker/routes/desktopAuth.ts:164` | GET | `${DESKTOP_AUTH}/capabilities` |
| `worker/routes/desktopAuth.ts:169` | POST | `${DESKTOP_AUTH}/requests` |
| `worker/routes/desktopAuth.ts:230` | POST | `${DESKTOP_AUTH}/prepare` |
| `worker/routes/desktopAuth.ts:249` | POST | `${DESKTOP_AUTH}/authorize` |
| `worker/routes/desktopAuth.ts:266` | POST | `${DESKTOP_AUTH}/poll` |
| `worker/routes/desktopAuth.ts:279` | POST | `${DESKTOP_AUTH}/cancel` |
| `worker/routes/desktopAuth.ts:289` | POST | `${DESKTOP_AUTH}/exchange` |
| `worker/routes/deviceToken.ts:15` | POST | `/api/assist/device-token` |
| `worker/routes/devices.ts:19` | GET | `/api/auth/devices` |
| `worker/routes/devices.ts:58` | POST | `/api/auth/devices/revoke` |
| `worker/routes/docPreviews.ts:22` | GET | `/api/settings/invoice-preview` |
| `worker/routes/employees.ts:622` | GET | `/api/employees` |
| `worker/routes/employees.ts:691` | GET | `/api/employees/:id` |
| `worker/routes/employees.ts:703` | POST | `/api/employees` |
| `worker/routes/employees.ts:774` | PUT | `/api/employees/:id` |
| `worker/routes/employees.ts:855` | POST | `/api/employees/:id/resign` |
| `worker/routes/employees.ts:943` | POST | `/api/employees/:id/reactivate` |
| `worker/routes/employees.ts:960` | POST | `/api/employees/parse-id` |
| `worker/routes/employees.ts:1024` | POST | `/api/employees/:id/id-document` |
| `worker/routes/employees.ts:1120` | GET | `/api/employees/:id/id-document` |
| `worker/routes/employees.ts:1150` | POST | `/api/employees/:id/advance` |
| `worker/routes/employees.ts:1261` | POST | `/api/employees/:id/pay` |
| `worker/routes/employees.ts:1355` | DELETE | `/api/employees/:id` |
| `worker/routes/expenses.ts:73` | POST | `/api/expenses` |
| `worker/routes/expenses.ts:119` | POST | `/api/expenses/import` |
| `worker/routes/finance.ts:79` | GET | `/api/admin/finance/monthly` |
| `worker/routes/finance.ts:112` | GET | `/api/admin/employees/payroll` |
| `worker/routes/finance.ts:197` | GET | `/api/me/payroll` |
| `worker/routes/finance.ts:296` | GET | `/api/admin/finance/statement` |
| `worker/routes/finance.ts:323` | GET | `/api/admin/finance/period` |
| `worker/routes/finance.ts:345` | GET | `/api/admin/finance/monthly/report.pdf` |
| `worker/routes/finance.ts:405` | POST | `/api/cron/finance-reminders` |
| `worker/routes/invoices.ts:5` | GET | `/api/invoices` |
| `worker/routes/invoices.ts:36` | GET | `/api/invoices/:id` |
| `worker/routes/mailbox.ts:494` | GET | `/api/admin/agent/threads` |
| `worker/routes/mailbox.ts:516` | GET | `/api/admin/agent/threads/:id` |
| `worker/routes/mailbox.ts:532` | PUT | `/api/admin/agent/threads/:id` |
| `worker/routes/mailbox.ts:555` | DELETE | `/api/admin/agent/threads/:id` |
| `worker/routes/partner-v1.ts:128` | GET | `/api/v1/openapi.json` |
| `worker/routes/partner-v1.ts:133` | GET | `/api/v1/health` |
| `worker/routes/partner-v1.ts:147` | GET | `/api/v1/products` |
| `worker/routes/partner-v1.ts:172` | GET | `/api/v1/products/:id` |
| `worker/routes/partner-v1.ts:185` | GET | `/api/v1/clients` |
| `worker/routes/partner-v1.ts:210` | GET | `/api/v1/invoices` |
| `worker/routes/partner-v1.ts:234` | POST | `/api/v1/invoices` |
| `worker/routes/public.ts:30` | GET | `/api/public/github-profile` |
| `worker/routes/stock.ts:30` | GET | `/api/products/:id/movements` |
| `worker/routes/stock.ts:58` | GET | `/api/stock/movements` |
| `worker/routes/stockAdjustments.ts:177` | POST | `/api/stock-adjustment-requests` |
| `worker/routes/stockAdjustments.ts:253` | GET | `/api/stock-adjustment-requests` |
| `worker/routes/stockAdjustments.ts:272` | GET | `/api/stock-adjustment-requests/:id` |
| `worker/routes/stockAdjustments.ts:393` | POST | `/api/stock-adjustment-requests/:id/approve` |
| `worker/routes/stockAdjustments.ts:394` | POST | `/api/stock-adjustment-requests/:id/reject` |
| `worker/routes/wholesale.ts:30` | GET | `/api/clients/:id/prices` |
| `worker/routes/wholesale.ts:45` | PUT | `/api/clients/:id/prices/:productId` |
| `worker/routes/wholesale.ts:71` | GET | `/api/clients/:id/price-for/:productId` |
| `worker/routes/wholesale.ts:81` | GET | `/api/inventory/reorder-suggestions` |
| `worker/routes/workStatus.ts:18` | GET | `/api/mobile/work-status` |

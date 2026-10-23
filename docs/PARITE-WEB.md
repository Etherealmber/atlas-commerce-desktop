# Parité avec le ERP web Atlas Commerce

État de référence généré le 2026-10-23 à partir du dépôt `Etherealmber/atlas-commerce`
(branche `main`) : `worker/index.ts` + `worker/routes/*.ts` font foi pour le
contrat d'API, et `src/pages/` pour les écrans. Le client desktop ne consomme
que ces endpoints — aucun contrat n'est deviné.

## Couverture actuelle

- **328 endpoints** uniques côté worker (méthode + chemin, identifiants
  normalisés en `:id`).
- Le desktop en couvre **26**, soit environ 8 %. Tous les endpoints appelés
  par le client existent à l'identique dans le worker : aucun appel hors
  contrat.
- Écrans web : **32 pages** dans `src/pages/` ; écrans desktop natifs :
  connexion (avec OTP, lien magique et session QR), tableau de bord, produits,
  clients (CRM), fournisseurs, ventes/factures (liste), notifications, agent,
  paramètres, coquille.

## Matrice par domaine

| Domaine | Couverts | Total |
|---|--:|--:|
| `admin` | 0 | 39 |
| `auth` | 9 | 29 |
| `invoices` | 1 | 24 |
| `inventory` | 0 | 17 |
| `reports` | 0 | 17 |
| `products` | 3 | 16 |
| `bills` | 0 | 15 |
| `employees` | 0 | 14 |
| `public` | 0 | 13 |
| `clients` | 3 | 12 |
| `settings` | 0 | 11 |
| `attendance` | 0 | 9 |
| `receipts` | 0 | 9 |
| `catalog` | 0 | 8 |
| `assist` | 1 | 7 |
| `stock` | 0 | 7 |
| `v1` (API partenaire externe) | 0 | 7 |
| `export` | 0 | 6 |
| `integrations` | 0 | 6 |
| `mobile-actions` | 0 | 6 |
| `vendors` | 3 | 6 |
| `expenses` | 0 | 5 |
| `categories` | 0 | 4 |
| `units` | 0 | 4 |
| `agent` | 1 | 3 |
| `delivery-notes` | 0 | 3 |
| `kits` | 0 | 3 |
| `webhooks` | 0 | 3 |
| `atlas` | 0 | 2 |
| `fiscal` | 0 | 2 |
| `import` | 0 | 2 |
| `integrity` | 0 | 2 |
| `ledger` | 0 | 2 |
| `scan` | 0 | 2 |
| `alerts` | 1 | 1 |
| `bank` | 0 | 1 |
| `canary` | 1 | 1 |
| `cron` | 0 | 1 |
| `dashboard` | 1 | 1 |
| `health` | 1 | 1 |
| `hub` | 0 | 1 |
| `me` | 0 | 1 |
| `media` | 0 | 1 |
| `nav-badges` | 1 | 1 |
| `print` | 0 | 1 |
| `push` | 0 | 1 |
| `assetlinks.json` | 0 | 1 |

## Écrans web et sort desktop

| Page web (`src/pages/`) | Équivalent desktop |
|---|---|
| `Login`, `LoginQr` | Partiel — connexion OTP/lien/QR déjà présente ; manque webauthn |
| `Dashboard` | Oui |
| `Products`, `ProductHistory` | Partiel — lecture + ajustement de stock ; manque CRUD, variantes, images, historique mouvements |
| `Invoices`, `InvoiceDetail` | Partiel — liste seule ; manque détail, création, paiements, statuts, PDF/UBL |
| `Quotes` | Non (devis) |
| `QuickSale`, `QuickSaleDetail` | Non (vente comptoir) |
| `Receipts` | Non (reçus) |
| `Clients` | Partiel — liste, recherche, solde dû, création et modification ; manque prix par client, imports et suppressions OTP |
| `Vendors`, `Bills` | Partiel — fournisseurs en liste/recherche/création/modification ; les factures fournisseurs (`Bills`) restent à couvrir |
| `InventoryOps` | Non (opérations de stock multi-emplacements) |
| `Employees`, `Attendance` | Non (RH et pointage) |
| `Reports`, `Balance` | Non (17 rapports + balance) |
| `Bank` | Non (rapprochement CSV) |
| `Reimbursements` | Non (remboursements, rattachés aux factures) |
| `Scan` | Non (lecture code-barres) |
| `ImportData` | Non (imports validés en deux temps) |
| `Actions` | Non (file d'actions mobile) |
| `Settings` | Partiel — réglages locaux seuls ; manque les réglages serveur |
| `AccessPage` | Non (administration des utilisateurs) |
| `About`, `Welcome`, `WhatsNew`, `Privacy`, `Terms`, `EmailVerify` | Non — pages web ; à traiter en panneau natif ou à exclure |

## Exclusions proposées (non transposables au desktop)

- `cron/*`, `.well-known/assetlinks.json`, `push/register`,
  `auth/login-biometric`, `public/*` (portail client sans session) : liés au
  navigateur, à Android ou aux tâches planifiées du worker.
- `v1/*` (`partner-v1`) : API ouverte pour intégrations externes, pas un
  écran de l'application.
- `print/thermal/*` : l'impression thermique dépend d'un imprimant réseau ;
  à réévaluer après les écrans métier.

## Feuille de route par phases

Chaque phase ajoute endpoints + modèles typés + écran egui monochrome +
entrée de navigation, et ne modifie jamais le contrat du worker.

1. **Vente et CRM** — `clients` (liste, fiche, CRUD, prix par client),
   `invoices` (création, détail, paiements, statuts, PDF), `Quotes`,
   `QuickSale`, `receipts`, `delivery-notes`. *Premier livrable livré :
   écran Clients avec liste, recherche, solde dû, création et modification
   (`GET/POST/PUT /api/clients`). Restent les prix par client, les imports
   et les suppressions OTP.*
2. **Catalogue et stock** — CRUD `products` avec variantes et images
   (`catalog`), `categories`, `units`, `kits`, `inventory` (emplacements,
   lots, inventaires cycliques, réappro, valorisation), `scan`,
   historique des mouvements.
3. **Achats** — `vendors`, `bills` (réception, paiements, rapprochement),
   `expenses`, demandes de stock. *Le CRUD fournisseurs
   (`GET/POST/PUT /api/vendors`) est déjà livré ; restent les suppressions
   à confirmation de mot de passe, les factures et les dépenses.*
4. **RH** — `employees` (embauche, documents, avances, paie), `attendance`
   (pointage, absences, exports), formulaires imprimables.
5. **Finance et pilotage** — les 17 `reports`, `balance`, `bank`
   (rapprochement CSV), `ledger`, `fiscal` (clôture de période), `export/*`.
6. **Administration et réglages** — `admin` (utilisateurs, audit,
   sauvegardes, configuration, clés d'intégration, webhooks), `settings`
   serveur, `integrity`, `import` de données, `mobile-actions`.
7. **Compléments d'authentification** — webauthn, appareils de confiance,
   avatars, et les écrans natifs remplaçant About/WhatsNew.

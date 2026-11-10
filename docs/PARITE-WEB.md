# Parité avec le ERP web Atlas Commerce

État de référence généré le 2026-10-23 à partir du dépôt `Etherealmber/atlas-commerce`
(branche `main`) : `worker/index.ts` + `worker/routes/*.ts` font foi pour le
contrat d'API, et `src/pages/` pour les écrans. Le client desktop ne consomme
que ces endpoints — aucun contrat n'est deviné.

## Audit fonctionnel web de référence

L'audit fourni par l'utilisateur porte sur le web au commit
`cf951397553dc8749f15c96b33abf02d4a360e35` (analyse statique, sans captures
ni test natif). Il complète l'inventaire d'endpoints ci-dessous ; ses
recommandations P0–P2 ne sont pas des comportements observés du web.

- Une vente rapide créée n'est pas nécessairement encaissée : le desktop
  l'indique et vérifie chaque ligne avant l'envoi. L'encaissement séparé n'est
  pas encore implémenté dans l'écran natif.
- Le web déclenche le bon de livraison depuis une facture. Le desktop garde
  son écran et son créateur de BL autonome existants : ce n'est pas une parité
  du parcours facture → BL.
- Les dépenses sont une section de « Solde & mouvements » sur le web ; le
  desktop conserve actuellement son écran distinct. Notifications restent une
  page native ; l'agent s'ouvre désormais dans un panneau latéral droit.
- Les rôles web `owner`, `admin` et `secretaire` ont accès aux tâches bureau ;
  `stock` est orienté produits/scan et ne voit pas les montants. Le desktop
  masque les vues financières et les prix aux magasiniers, y compris dans la
  palette et les raccourcis. Par prudence, les formulaires de création produit
  natifs contenant des prix leur sont masqués ; l'ajustement du stock reste
  disponible. Les contrôles d'interface ne remplacent jamais les permissions
  côté serveur ; des tests sur serveur réel restent nécessaires.
- Les réglages desktop restent des cartes, pas les 13 onglets web. L'audit
  recommande une gestion cohérente des dialogues (focus, superposition,
  brouillons et opérations en cours), mais ces garanties ne sont pas encore
  généralisées à tous les formulaires natifs.

Les chiffres de couverture ci-dessous reflètent l'inventaire à sa date de
rédaction, pas une nouvelle mesure effectuée lors de cet audit.

## Couverture actuelle

- **328 endpoints** uniques côté worker (méthode + chemin, identifiants
  normalisés en `:id`).
- Le desktop en couvre **44**, soit environ 13 %. Tous les endpoints appelés
  par le client existent à l'identique dans le worker : aucun appel hors
  contrat. Le ticket `GET /api/reports/link-ticket` sert à ouvrir les
  rapports imprimables dans le navigateur système sans exposer le JWT.
- Écrans web : **32 pages** dans `src/pages/` ; écrans desktop natifs :
  connexion (avec OTP, lien magique et session QR), tableau de bord, produits
  (avec barre de scan code-barres/SKU et création produit rapide),
  catalogue (catégories et unités), clients (CRM), fournisseurs,
  dépenses/recettes, bons de livraison, ventes/factures (liste),
  notifications, agent, paramètres, coquille.

## Matrice par domaine

| Domaine | Couverts | Total |
|---|--:|--:|
| `admin` | 0 | 39 |
| `auth` | 10 | 29 |
| `invoices` | 4 | 24 |
| `inventory` | 0 | 17 |
| `reports` | 1 | 17 |
| `products` | 3 | 16 |
| `bills` | 2 | 15 |
| `employees` | 7 | 14 |
| `public` | 0 | 13 |
| `clients` | 3 | 12 |
| `settings` | 1 | 11 |
| `attendance` | 0 | 9 |
| `receipts` | 2 | 9 |
| `catalog` | 0 | 8 |
| `assist` | 1 | 7 |
| `stock` | 0 | 7 |
| `v1` (API partenaire externe) | 0 | 7 |
| `export` | 0 | 6 |
| `integrations` | 3 | 6 |
| `mobile-actions` | 0 | 6 |
| `vendors` | 3 | 6 |
| `expenses` | 2 | 5 |
| `categories` | 3 | 4 |
| `units` | 3 | 4 |
| `agent` | 1 | 3 |
| `delivery-notes` | 2 | 3 |
| `kits` | 0 | 3 |
| `webhooks` | 0 | 3 |
| `atlas` | 0 | 2 |
| `fiscal` | 0 | 2 |
| `import` | 0 | 2 |
| `integrity` | 0 | 2 |
| `ledger` | 0 | 2 |
| `scan` | 2 | 2 |
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
| `Invoices`, `InvoiceDetail` | Partiel — liste, détail natif, impression et PDF officiels ouverts dans le navigateur via un ticket `rt` de 30 minutes ; manque création, paiements, statuts et UBL |
| `Quotes` | Non (devis) |
| `QuickSale`, `QuickSaleDetail` | Non (vente comptoir) |
| `Receipts` | Partiel — liste en lecture seule, ouverture de la facture liée et impression officielle via ticket ; création et suppression restent à couvrir |
| `Clients` | Partiel — liste, recherche, solde dû, création et modification ; `Factures` affiche une liste filtrée par identifiant dans l'application avec option navigateur. `Situation` ouvre encore le rapport imprimable via ticket `link-ticket` ; manque détail de facture, prix par client, imports et suppressions OTP |
| `Vendors`, `Bills` | Partiel — fournisseurs en liste/recherche/création/modification ; factures fournisseurs en lecture seule avec impression/PDF officiels. Création, paiements, réception et rapprochement restent à couvrir |
| `InventoryOps` | Non (opérations de stock multi-emplacements) |
| `Employees`, `Attendance` | Partiel — liste, recherche, statut et catégorie, détail avec historique des paiements, paiement, acompte, démission et réactivation, fiche d'embauche et situation RH imprimables ; création réduite aux champs essentiels. Manquent édition complète, pièce d'identité chiffrée et pointage |
| `Reports`, `Balance` | Partiel — `client-situation` ouvert dans le navigateur via `link-ticket` ; les 16 autres rapports et la balance restent à couvrir |
| `Bank` | Non (rapprochement CSV) |
| `Reimbursements` | Non (remboursements, rattachés aux factures) |
| `Scan` | Oui — barre de scan dans Produits : recherche code-barres/SKU (`GET /api/scan/barcode/:code`) et création produit rapide avec gestion du 409 `DUPLICATE_PRODUCT` (`POST /api/scan/quick-product`) |
| `ImportData` | Non (imports validés en deux temps) |
| `Actions` | Non (file d'actions mobile) |
| `Settings` | Partiel — réglages locaux seuls ; manque les réglages serveur |
| `AccessPage` | Non (administration des utilisateurs) |
| Documents imprimables | Les pages HTML officielles du serveur (facture, PDF, reçu, facture fournisseur, fiche d'embauche, situation RH) s'ouvrent dans le navigateur système avec un ticket `rt` de 30 minutes, limité par le serveur aux chemins d'impression. Le jeton de session n'apparaît jamais dans l'URL ; aucun rendu local ne prétend remplacer ces pages. |

| `About`, `Welcome`, `WhatsNew`, `Privacy`, `Terms`, `EmailVerify` | Partiel — le menu profil propose `Nouveautés` (notes de version dans les réglages, option navigateur) et `À propos` (section réglages) ; les pages juridiques et Welcome restent à exclure |

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
   historique des mouvements. *Catégories et unités déjà livrées
   (liste, création, modification ; les suppressions à confirmation de mot
   de passe restent à venir). Produits : lecture, recherche, création et
   ajustement de stock déjà livrés ; variantes, images et suppressions à
   venir.*
3. **Achats** — `vendors`, `bills` (réception, paiements, rapprochement),
   `expenses`, demandes de stock. *Le CRUD fournisseurs
   (`GET/POST/PUT /api/vendors`) est déjà livré ; restent les suppressions
   à confirmation de mot de passe, les factures et les dépenses.*
4. **RH** — `employees` (embauche, documents, avances, paie), `attendance`
   (pointage, absences, exports), formulaires imprimables.
5. **Finance et pilotage** — les 17 `reports`, `balance`, `bank`
   (rapprochement CSV), `ledger`, `fiscal` (clôture de période), `export/*`.
6. **Administration et réglages** — `admin` (utilisateurs, audit,
   sauvegardes, configuration, webhooks), `settings` serveur, `integrity`,
   `import` de données, `mobile-actions`. *Les clés d'intégration
   (`GET/POST/DELETE /api/integrations/keys`) et l'état du fournisseur
   d'assistance (`GET /api/settings/assist-provider`) sont livrés ; restent
   les webhooks et la sauvegarde/restauration.*
7. **Compléments d'authentification** — webauthn, appareils de confiance,
   et les écrans natifs remplaçant About/WhatsNew. *L'édition du profil
   (`PATCH /api/auth/me`, nom affiché, e-mail, téléphone, note, URL d'avatar)
   est livrée ; le mot de passe reste verrouillé côté serveur.*

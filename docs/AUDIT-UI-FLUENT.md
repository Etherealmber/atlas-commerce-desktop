# Audit et progression de l'interface desktop

## Périmètre inspecté

L'application est native Rust/egui avec fastframe, et non Tauri/React :
`Cargo.toml`, `crates/core/src/settings.rs`, `crates/ui/src/theme.rs`,
`fonts.rs`, `widgets.rs`, `screens/{shell,dashboard,auth}.rs`,
`shortcuts.rs`, `app.rs`, les hôtes WebView Windows/macOS et le helper
`crates/webview-linux/src/window.rs`, ainsi que les fixtures et workflows CI.

| Écran / composant existant | Place dans la hiérarchie visée | État |
| --- | --- | --- |
| Démarrage, configuration serveur, connexion et mise à jour | Portails avant le shell | La connexion hébergée reste la page du serveur ; état natif de récupération réorganisé. |
| Shell, barre de titre, profil, notifications | Chrome et commandes | Rail resserré et regroupé ; recherche métier compacte conservée, palette clavier existante. |
| Tableau de bord | Indicateurs, analyses, détails repliables | Quatre indicateurs existants, graphe et impayés conservés ; factures récentes repliées. |
| Produits, catalogue, stock, fournisseurs | Catalogue et stock | Couleurs et composants partagés mis à jour ; révision détaillée à poursuivre. |
| Ventes, clients, livraisons | Opérations | Navigation regroupée ; écrans métier et permissions conservés. |
| Dépenses, employés | Gestion | Navigation regroupée ; écrans métier conservés. |
| Notifications, réglages, agent | Fonctions secondaires | Accès existants conservés ; révision détaillée à poursuivre. |
| Palette et dialogues | Surfaces superposées | Les raccourcis et les actions existants sont conservés ; audit complet d'accessibilité à poursuivre. |

## Limites et décisions

Le thème clair devient le défaut des nouvelles configurations, sans écraser un
choix de thème enregistré. Le mode sombre et les palettes personnalisées
restent disponibles. Les surfaces principales claires sont opaques ; les
ombres, les masques d'icônes et les voiles modaux utilisent encore l'alpha.
La demande d'un voile semi-transparent contredit une interdiction absolue de
toute transparence. Aucun effet de verre dépoli n'est ajouté.

La connexion intégrée démarre automatiquement dès la fin de restauration du
coffre pour un serveur vérifié ; pendant la création de la demande v1, un statut
minimal et annulable remplace l'écran d'attente décoratif. Sur Windows et macOS,
la barre de sécurité réserve moins d'espace au-dessus de la page. La vérification
des capacités et la création de la demande restent indispensables avant
l'ouverture de l'URL validée ; la rapidité dépend aussi du serveur.

Le contenu du formulaire de connexion intégré est servi par le serveur choisi :
il n'est ni copié ni modifié par injection CSS/JS. Le cadre natif et la
récupération peuvent être améliorés ici ; changer le design de ce formulaire
nécessite une modification du projet web et ses propres tests d'authentification.
Les fixtures fictives ne démontrent pas le fonctionnement réel de Turnstile,
passkeys, QR ou impression, ni l'accessibilité complète.

Les données et actions fictives ne remplacent pas les opérations réelles :
aucune checklist persistante, variation financière ou action de traitement de
factures n'est inventée. Le regroupement complet des écrans, la palette avec
recherche métier intégrée, les états de focus et les chiffres tabulaires dans
tous les tableaux restent à auditer et implémenter.

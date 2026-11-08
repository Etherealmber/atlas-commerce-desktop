# Questions pour harmoniser l’application desktop avec le web

Copier ce prompt dans une conversation disposant du dépôt `atlas-commerce` :

> Analyse le code actuel de l’interface web Atlas Commerce et décris son
> comportement réel, sans modifier le Worker ni inventer d’API, pour guider
> une implémentation native egui sur Windows/macOS/Linux.
>
> Pour chaque page (tableau de bord, produits, clients, fournisseurs,
> dépenses, ventes, bons de livraison, notifications, agent et réglages),
> précise :
>
> 1. Les composants et chemins source qui font autorité, et l’ordre visuel
>    des actions et informations ; les différences selon les rôles.
> 2. Les dimensions, marges, hiérarchie typographique, couleurs clair/sombre,
>    styles de boutons et icônes, et seuils de mise en page responsive.
> 3. Les états chargement, vide, erreur, validation, absence de permission et
>    confirmation. Indique les textes français et raccourcis si disponibles.
> 4. Pour chaque dialogue ou popover : déclencheur, largeur, défilement,
>    boutons, validation, fermeture par Échap/clic extérieur, et protection
>    des changements non enregistrés. Signale les éventuelles différences
>    avec les composants partagés `AppDialogs` et `ModalFormFeedback`.
> 5. La barre supérieure : ordre des contrôles, recherche, navigation,
>    notifications, menu utilisateur, comportement étroit et zone de titre.
> 6. Les onglets et sections des réglages, leurs permissions, états de
>    sauvegarde et actions exactes. Ne demande ni mot de passe, jeton ni
>    donnée personnelle, et ne reproduis aucun secret.
>
> Cite les chemins/fonctions pertinents, distingue comportement observé et
> recommandation, et fournis une liste priorisée des corrections desktop.
> Mentionne les limites lorsque les captures ou les données réelles manquent.

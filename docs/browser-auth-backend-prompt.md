# Proposition pour une connexion navigateur native complète

Le serveur actuel prend déjà en charge une approbation QR d’un ordinateur :
`POST /api/auth/qr/pc-session`, puis la page `/login/qr?t=…&mode=pc`, le
suivi du statut et l’échange final. La page d’approbation exige toutefois une
session web déjà ouverte : si ce n’est pas le cas, elle demande de se
connecter puis de recommencer. Le code expire au bout de 60 secondes. Le
client desktop peut utiliser ce mécanisme aujourd’hui, mais ce n’est pas une
redirection de connexion navigateur entièrement automatisée.

## Prompt pour le dépôt Atlas Commerce

> Ajouter une connexion sécurisée depuis un client desktop natif au dépôt
> Atlas Commerce, sans navigateur intégré et sans modifier les connexions
> existantes. Chaque utilisateur choisit son propre serveur auto-hébergé :
> ne pas coder en dur de domaine de production.
>
> Étudier les endpoints QR PC existants et la liaison actuelle des sessions à
> l’IP et à `X-Device-Fingerprint`. Privilégier leur réutilisation si elle
> permet de satisfaire les exigences ci-dessous. Sinon proposer un contrat
> versionné et documenté, plutôt que de supposer qu’un endpoint existe déjà.
>
> 1. Le desktop crée une demande de courte durée associée à une preuve
>    générée par le client (par exemple PKCE S256), à un état aléatoire et à
>    l’origine exacte du serveur sélectionné. La preuve secrète reste dans le
>    processus desktop ; ne pas exposer de jeton de session dans une URL.
> 2. Le client ouvre le navigateur système. Si nécessaire, l’utilisateur se
>    connecte via le parcours web existant (y compris OTP et protections web),
>    puis revient à l’écran explicite d’autorisation du desktop. Préserver la
>    demande pendant la connexion sans créer de redirection ouverte.
> 3. L’autorisation doit exiger une action explicite, vérifier CSRF, expirer
>    rapidement, être à usage unique et résister à la substitution de demande
>    ou d’origine. Elle ne doit pas authentifier automatiquement un autre
>    appareil simplement parce qu’un lien a été ouvert.
> 4. Le desktop récupère le résultat par échange sécurisé ou polling borné ;
>    la preuve doit être vérifiée au moment de l’échange. Émettre une session
>    liée au fingerprint du desktop, et non à celui du navigateur. Ne pas
>    exposer de jeton longue durée dans l’historique, les journaux, un
>    référent HTTP, ou une URL de callback.
> 5. Limiter les tentatives, prévoir refus, expiration et annulation, et
>    documenter les messages d’erreur ; ne pas affaiblir la vérification IP ou
>    d’appareil existante. Les anciens serveurs doivent rester utilisables via
>    la connexion native et le QR actuel.
> 6. Ajouter tests unitaires et d’intégration : connexion web déjà ouverte et
>    non ouverte, serveur auto-hébergé, OTP, refus, expiration, rejeu,
>    mauvais état/preuve/origine, usurpation de callback, et absence de secrets
>    dans les journaux. Fournir le contrat exact des requêtes/réponses pour
>    l’implémentation desktop ultérieure.

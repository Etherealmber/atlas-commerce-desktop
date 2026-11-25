# Demande backend : découverte des mises à jour Desktop via Atlas API

## Constat vérifié

Le client Android (`atlas-commerce-android`, révision `0c2b4aa`) interroge l'origine Atlas configurée avec `GET /api/app/android-update` et recherche également un manifeste GitHub. Le Worker `atlas-commerce` révision `3b0e8c6` implémente cette route dans `worker/routes/androidRelease.ts` : elle est propre à Android (`versionCode`, `apkUrl`, politique minimale Android et objets R2 `app-updates/android/*`). Ce contrat ne décrit ni les architectures ni les installateurs Desktop. Il ne faut pas l'appeler depuis Desktop ni interpréter un APK comme une mise à jour Desktop.

## Origine prévue par le propriétaire

Le propriétaire souhaite utiliser `https://cdn1.afmarbre.com` pour la découverte des mises à jour Atlas Desktop. Ce choix ne confirme pas qu'une route, un manifeste, un certificat, une configuration CORS ou des artefacts y existent déjà. L'accès doit être limité à une origine HTTPS approuvée et indépendante de l'origine métier installée ; ne lui envoyer aucun secret de session. Le contrat exact, l'hébergement, les signatures et la politique de mise à jour restent à établir avant de brancher le client. La vérification GitHub existante reste active en attendant.

## Travail demandé au développeur backend

Proposer et publier un contrat Desktop séparé, puis transmettre au développeur Desktop le nom exact de route, ses schémas JSON et ses règles d'erreur avant intégration. Ne pas déployer sans approbation explicite. Le contrat devrait :

1. Être consultable avant connexion sur l'origine HTTPS Atlas déjà configurée, sans identifiants de session ; distinguer explicitement Desktop, version courante, canal stable/préversion, OS, architecture et type d'installation. Des versions ou plateformes inconnues doivent produire une erreur claire, jamais un APK ou une mise à jour d'une autre plateforme.
2. Retourner de manière bornée et non ambiguë les métadonnées réelles de version, notes, canal et politique minimale, un état explicite lorsqu'aucune version n'est publiée et une liste d'artefacts Desktop compatibles, avec taille et SHA-256. Préciser le versionnage, les règles de comparaison semver et d'expiration/cache, y compris les réponses 404, 429 et absence de manifeste.
3. Définir une chaîne de confiance des métadonnées et binaires (signature vérifiée par une clé publique épinglée et stratégie de rotation, ou contrat de vérification GitHub signé équivalent). Les URLs d'artefacts doivent être HTTPS, contraintes à des origines approuvées et vérifiées après redirection. Ne jamais transmettre tokens, empreinte, mot de passe ou autres identifiants à une origine tierce.
4. Documenter la résolution des divergences entre Atlas API et GitHub, les politiques obligatoires et le comportement hors ligne. Une panne du serveur Atlas ne doit pas être interprétée comme « à jour ». Éviter une politique obligatoire non authentifiée capable de bloquer indéfiniment l'application.
5. Fournir la procédure d'administration et de publication des métadonnées et des artefacts Windows/macOS/Linux par canal, avec rôles, audit, stockage, secrets nécessaires, checksum/signature, conservation, rollback, et tests de non-régression Android. Ne pas créer ni publier de version fictive.
6. Tester les clients sans connexion, toutes plateformes/canaux, version plus ancienne/identique/plus récente, signature incorrecte, manifestes expirés ou incohérents, URLs non sûres, redirections, absence, réponses malformées ou surdimensionnées, 429, panne et variantes de cache. Vérifier explicitement qu'aucune réponse Android n'est acceptée comme Desktop.

L'application Desktop actuelle continue à utiliser ses GitHub Releases. Aucun binaire ne doit être téléchargé automatiquement ; la sélection et le téléchargement/install restent une action explicite. Un checksum seul ne prouve pas l'origine de l'éditeur.

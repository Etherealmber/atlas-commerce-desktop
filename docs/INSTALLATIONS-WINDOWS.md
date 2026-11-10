# Installations Windows en double : diagnostic prudent

Une ancienne installation MSI et l'installeur Inno peuvent coexister. Ce ne
sont pas deux mises à jour d'un même produit : leurs identités et leurs
uninstallers sont différents. Le filtre de l'updater Inno évite de choisir
un nouvel MSI, mais **ne supprime pas** les copies déjà présentes.

## Avant toute suppression

1. Fermer Atlas Commerce et vérifier qu'il n'est pas réduit dans la zone de
   notification. Faire une sauvegarde des données et réglages locaux avant
   toute intervention. Ne pas supprimer les dossiers de données.
2. Dans Paramètres Windows → Applications installées, examiner les entrées
   « Atlas Commerce » ou « Atlas Commerce Desktop ». Deux raccourcis ne
   prouvent pas à eux seuls que deux installations existent.
3. Déterminer les dossiers et le type de chaque installation avant d'agir.
   L'identité Inno connue se termine par
   `{E74305A7-4960-45B6-93C9-CEA8D83B1517}_is1` dans les clés de
   désinstallation ; les MSI utilisent leurs propres codes de produit.
   Les clés peuvent être dans HKCU ou HKLM et dans différentes vues 32/64 bits.
   Ne pas supposer qu'une entrée portant le même nom est nécessairement Atlas.
4. Si les dossiers se recouvrent, si l'identité ou l'origine d'une entrée est
   incertaine, si des droits administrateur sont nécessaires, ou si une
   sauvegarde n'est pas disponible : **arrêter** et faire vérifier la situation.
   Ne pas effacer de dossier, clé ou raccourci à la main et ne pas lancer une
   commande de désinstallation copiée d'une source non vérifiée.

## Cas contrôlé : dossiers MSI et Inno distincts

Sur une machine de test jetable, la CI tente les opérations suivantes :
installer les deux paquets dans des dossiers distincts, désinstaller le MSI,
réexécuter le même installeur Inno dans son dossier et vérifier que son
exécutable et son association `atlascommerce://` subsistent. Les deux
installateurs peuvent écrire des associations communes : la réparation Inno
après désinstallation est donc importante. Vérifier également le raccourci et
la connexion après intervention. Le résultat du job Windows packaging indique
si cette recette a réussi ; ce n'est pas un test sur l'installation réelle de
l'utilisateur, ni une preuve pour des dossiers partagés, tous anciens MSI,
l'updater réseau, ou des installations système.

Aucune migration automatique destructive n'est activée. Pour un appareil
réel, utiliser uniquement des paquets Atlas vérifiés et demander de l'aide
avant suppression si les identités, dossiers ou conséquences ne sont pas
clairs. Ne jamais partager de jeton ou de fichier de session pour ce diagnostic.

## Serveur choisi à l'installation

L'installeur Inno demande l'adresse du serveur avant de copier les fichiers.
La page propose `https://stock.afmarble.com`, accepte une adresse HTTPS
(ou `http://localhost` pour un serveur local) et refuse espaces, guillemets,
barre oblique inverse et identifiants. Une installation silencieuse peut la
fournir avec `/SERVERURL=https://exemple.example`.

À la fin de l'installation, l'installeur écrit `server.json` à côté de
l'exécutable et mémorise l'adresse dans
`HKCU\Software\AtlasCommerceDesktop\Server`. Au premier lancement, si aucun
serveur n'est déjà verrouillé, l'application importe le fichier, le supprime
et verrouille cette adresse. Le réglage est ensuite en lecture seule :
l'interface explique qu'une **réinstallation** est nécessaire pour changer de
serveur. Lors d'une mise à jour, la page est préremplie avec l'adresse
mémorisée : un serveur auto-hébergé n'est donc jamais remplacé par la valeur
par défaut. La désinstallation supprime la valeur de registre, si bien qu'une
nouvelle installation repart de la page de choix.

Ce choix n'est pas un secret. Il ne remplace ni TLS, ni la vérification
d'origine, ni PKCE : l'application refuse toujours un document, un jeton ou
une demande d'autorisation liés à une autre origine. Un fichier `server.json`
invalide est ignoré et supprimé.

Sous Linux et macOS, sans page d'installation, le premier écran de
configuration verrouille le premier choix ; un déploiement géré peut fixer
l'adresse avant le premier lancement avec `ATLAS_SERVER_URL=https://…`.

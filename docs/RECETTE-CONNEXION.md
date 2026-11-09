# Recette manuelle de connexion — sans données sensibles

Cette recette est destinée à la personne qui possède la machine de test. Elle ne remplace pas les tests automatiques. **La WebView intégrée et le coffre système ne sont pas encore implémentés** : pour l'instant, seul le parcours v1 avec le navigateur système peut être testé ici. Ce parcours conserve actuellement la session uniquement en mémoire ; une fermeture complète impose donc de se reconnecter. Ne pas interpréter ce comportement comme une validation de la persistance demandée.

## Préparation

- Utiliser de préférence un serveur HTTPS et un compte de test sans données sensibles. Ne pas modifier la configuration de sécurité, désactiver Turnstile ou contourner l'OTP pour faire réussir le test.
- Sauvegarder ses données et éviter de désinstaller ou écraser une installation existante ; la migration d'installations Windows MSI/Inno déjà dupliquées n'est pas automatisée. Les installeurs de validation CI ne sont pas une release. Le montage universel de test CI embarque deux fois le binaire x64 : **il ne convient pas à ARM64**.
- Noter le commit testé, le système et sa version, ainsi que le navigateur et sa version. Ne pas communiquer l'adresse du serveur si elle est sensible.

## Parcours navigateur v1

1. Choisir le serveur et cliquer sur « Se connecter avec le navigateur ». Vérifier que le navigateur système s'ouvre et que le serveur affiché correspond au serveur choisi.
2. Suivre la politique normale du serveur : challenge Turnstile et OTP si demandés, ou passkey si elle est proposée. Ne pas communiquer le mot de passe, le code OTP, le QR, le code de comparaison ou les données de passkey.
3. Comparer le code affiché dans l'application et le navigateur, puis approuver explicitement. Vérifier l'accès à une page autorisée et indiquer si cela a fonctionné.
4. Dans une nouvelle tentative, refuser ou annuler avant l'approbation ; vérifier qu'aucune session ne s'ouvre. Tester l'expiration et le changement de serveur séparément. L'annulation locale n'est pas une preuve de révocation côté serveur.
5. Quitter complètement l'application, y compris la zone de notification, puis la rouvrir : noter le résultat. Le retour à l'écran de connexion est attendu pour la session v1 actuelle, faute de coffre et de restauration implémentés.

## En cas d'erreur

Indiquer le message exact et **l'étape** à laquelle il apparaît : avant l'ouverture du navigateur, challenge, OTP, consentement, attente ou après approbation. Les diagnostics du parcours v1 indiquent une étape fixe (`capabilities`, `requests`, `poll`, `exchange`), un statut HTTP et des catégories de contenu/échec. Si on les utilise, ne transmettre que ces champs, après vérification manuelle.

Ne jamais transmettre le journal complet, le corps d'une réponse, des identifiants, mots de passe, codes OTP, cookies, jetons, URL complètes ou captures du QR/de l'écran de consentement. Une capture de l'erreur seule peut être utile après masquage de toute information personnelle. Un résultat observé sur un OS ne prouve pas le fonctionnement sur les autres.

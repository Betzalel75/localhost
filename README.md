
---

# LOCALHOST

## Description
Ce projet est un serveur HTTP Rust qui permet de gérer plusieurs serveurs avec des configurations différentes. Le serveur prend en charge les requêtes HTTP, y compris le traitement des cookies, des fichiers multipart/form-data, et le routage basé sur une configuration définie dans un fichier `config.toml`.

## Fonctionnalités
- Gestion de plusieurs serveurs avec des configurations différentes.
- Routage basé sur des alias définis dans le fichier de configuration.
- Prise en charge des méthodes HTTP (GET, POST, DELETE).
- Gestion des cookies avec signature HMAC-SHA256 pour assurer l'intégrité.
- Support de la réception et du traitement des fichiers multipart/form-data.
- Configuration de la taille maximale du corps de la requête pour éviter les abus.
- Redirection des routes via des configurations spécifiques.
- Possibilité d'utiliser les cgi python et php. 

## Prérequis
- **Rust** (version 1.54 ou supérieure)
- **Cargo** pour la gestion des dépendances et l'exécution du projet

## Installation
Clonez le dépôt du projet à l'aide de la commande suivante :
```bash
git clone https://github.com/Betzalel75/localhost.git
cd localhost
```

## Configuration
La configuration du serveur se fait via un fichier `config.toml`. Ce fichier doit être placé à la racine du projet.

---

## Configuration du Fichier `config.toml`

### Structure Générale

```toml
[[servers]]
host_name = "localhost"          # Nom d'hôte du serveur (Obligatoire)
host = "127.0.0.1"               # Adresse IP du serveur (Obligatoire)
ports = [8080]                   # Liste des ports sur lesquels le serveur écoutera (Obligatoire)
root = "./www"                   # Chemin vers le répertoire racine où sont stockés les fichiers du serveur (Obligatoire)
error_pages = {                  # Pages d'erreur personnalisées (Optionnel)
  "404" = "./errors/404.html", 
  "500" = "./errors/500.html" 
}
client_body_limit = 1048576      # Limite de taille pour le corps des requêtes en octets (Optionnel, par défaut 1 MB)
directory_listing = true         # Activer ou désactiver l'affichage du contenu des répertoires (Optionnel, par défaut `false`)

[[servers.routes]]
alias = "/"                      # Alias de la route (Obligatoire)
pages = ["index.html", "about.html"]  # Liste des pages disponibles pour cette route (Obligatoire)
default_page = "index.html"      # Page par défaut si l'alias est accédé sans spécifier une page (Obligatoire)
check_cookie = true              # Activer la vérification des cookies pour cette route (Optionnel, par défaut `false`)
redirect = {                     # Redirections à appliquer pour cette route (Optionnel)
  "/old" = "/new"                # Une seule clé et une seule valeur sont acceptées
}
links = ["./links/link1", "./links/link2"]  # Liens relatifs pour charger des ressources supplémentaires (Optionnel)
methods = ["GET", "POST"]        # Méthodes HTTP acceptées pour cette route (Obligatoire)

[[servers.routes]]
alias = "/upload"
pages = ["upload.html"]
default_page = "upload.html"
check_cookie = false
methods = ["POST"]
```

### Détails des Paramètres

#### Paramètres du Serveur (`[[servers]]`)

- **`host_name` (Obligatoire)** : Nom d'hôte pour le serveur, par exemple, `"localhost"`. Ce nom est utilisé pour identifier le serveur.
  
- **`host` (Obligatoire)** : Adresse IP sur laquelle le serveur écoutera, par exemple `"127.0.0.1"`.

- **`ports` (Obligatoire)** : Liste des ports que le serveur utilisera pour écouter les connexions. Par exemple, `[8080, 8081]` permet au serveur d'écouter sur les ports 8080 et 8081.

- **`root` (Obligatoire)** : Chemin vers le répertoire racine du serveur, où sont stockés les fichiers accessibles par les clients. Par exemple, `"./www"`.

- **`error_pages` (Optionnel)** : Définissez des pages personnalisées pour les erreurs HTTP. Par exemple, `"404"` pour une erreur `Not Found`. Si ce champ est omis, le serveur utilise des messages d'erreur par défaut.

- **`client_body_limit` (Optionnel)** : Limite de la taille du corps de la requête en octets. Par exemple, `1048576` (1 MB). Si ce champ est omis, la valeur par défaut est de 1 MB.

- **`directory_listing` (Optionnel)** : Active (`true`) ou désactive (`false`) l'affichage du contenu des répertoires si une requête cible un répertoire. Par défaut, ce champ est désactivé (`false`).

#### Paramètres des Routes (`[[servers.routes]]`)

- **`alias` (Obligatoire)** : Définit l'alias de la route. C'est le chemin que l'utilisateur doit saisir dans l'URL. Par exemple, `"/"` pour la route racine.

- **`pages` (Obligatoire)** : Liste des pages disponibles pour cette route. Par exemple, `["index.html", "about.html"]`. Ces pages doivent se trouver dans le répertoire spécifié par `root`.

- **`default_page` (Obligatoire)** : Page par défaut à afficher si l'utilisateur accède à l'alias sans préciser de page spécifique. Par exemple, `"index.html"`.

- **`check_cookie` (Optionnel)** : Indique si les cookies doivent être vérifiés pour cette route. Valeur par défaut : `false`.

- **`redirect` (Optionnel)** : Définit les redirections pour cette route. Par exemple, `"/old" = "/new"`. Il est possible de spécifier une seule clé (l'ancienne route) et une seule valeur (la nouvelle route).

- **`links` (Optionnel)** : Liens supplémentaires pour charger des ressources. Par exemple, des chemins vers des fichiers CSS ou JavaScript. Exemples : `["./links/link1", "./links/link2"]`.

- **`methods` (Obligatoire)** : Méthodes HTTP acceptées pour cette route. Par exemple, `["GET", "POST"]`. Seules les méthodes spécifiées seront autorisées pour cette route.

---

Exemple de configuration `config.toml` :
```toml
[[servers]]
host_name = " localhost"
host = "127.0.0.1"
ports = [7878, 7879, 7880, 8080]
root = "/home/user/localhost/httpserver/public"
error_pages = { "404" = "404.html", "500" = "error.html" }
cgi_extensions = { "php" = "php-cgi.php", "py" = "python-cgi.py" }
client_body_limit = 1048576 # 10 MB
directory_listing = false


[[servers.routes]]
alias = "/singin/"
pages = ["password.html", "cookie.html"]
default_page = "/test.html"
links = ["/styles.css", "/css/style.css", "/css/dir.css"]
methods = ["GET", "POST", "DELETE"]
check_cookie = false
# redirect = {"/login/" = "login.html"} # redirect


[[servers.routes]]
alias = "/login/"
pages = ["login.html"]
default_page = "/login.html"
links = ["/styles.css", "/test.css"]
methods = ["GET", "POST"]
check_cookie = false


[[servers.routes]]
alias = "/"
pages = ["","index.html"] # Exception: write all pages without /
default_page = "/index.html"
links = ["/styles.css"]
methods = ["GET", "POST"]
check_cookie = false


[[servers.routes]]
alias = "/cookie/"
pages = [""] # Exception: write all pages without /
default_page = ""
links = [""]
methods = ["POST", "GET"]
check_cookie = false
redirect={"/"="index.html"}

[[servers.routes]]
alias = "/del/"
pages = ["",""] # Exception: write all pages without /
default_page = "/index.html"
links = ["/styles.css"]
methods = ["DELETE"]
check_cookie = false

```

## Lancer le Serveur
Pour lancer le serveur, utilisez la commande suivante :
```bash
cargo r -p httpserver
```

Le serveur démarrera et sera accessible sur les adresses configurées (par exemple, `http://127.0.0.1:8080`).

## Structure du Projet
```
├── Cargo.lock
├── Cargo.toml
├── config.toml
├── cookies.txt
├── http
│   ├── Cargo.toml
│   └── src
│       ├── httprequest.rs
│       ├── httpresponse.rs
│       └── lib.rs
├── httpserver
│   ├── Cargo.toml
│   ├── config.txt
│   ├── data
│   │   └── orders.json
│   ├── public
│   │   ├── 404.html
│   │   ├── cookie.html
│   │   ├── css
│   │   │   ├── dir.css
│   │   │   ├── password.html
│   │   │   └── style.css
│   │   ├── delete.txt
│   │   ├── dir.html
│   │   ├── error.html
│   │   ├── index.html
│   │   ├── login.html
│   │   ├── password.html
│   │   ├── php-cgi.php
│   │   ├── python-cgi.py
│   │   ├── styles.css
│   │   ├── test.css
│   │   └── test.html
│   └── src
│       ├── config.rs
│       ├── handler.rs
│       ├── lib.rs
│       ├── main.rs
│       ├── router.rs
│       └── server.rs
├── LICENSE
├── README.md
└── src
    └── main.rs
```

## Fonctionnement
- **Configuration** : Le fichier `config.toml` permet de définir les paramètres des serveurs, incluant les routes, les méthodes HTTP acceptées, et les pages d'erreurs personnalisées.
- **Gestion des Cookies** : Les cookies sont signés avec HMAC-SHA256 pour garantir leur intégrité et sont stockés dans un fichier `cookies.txt`.
- **Routage** : Le routage est basé sur les alias définis dans le fichier de configuration. Chaque route peut avoir des méthodes HTTP spécifiques et peut être associée à des redirections.
- **Multipart/Form-data** : Le serveur est capable de traiter les requêtes multipart/form-data, permettant ainsi l'upload de fichiers.

## Sécurité
- Les cookies sont signés avec un secret pour éviter les falsifications.
- La taille du corps des requêtes est limitée pour éviter les abus.

## Contributions

Les contributions au projet sont les bienvenues. Pour contribuer, suivez ces étapes :

1. Fork du repository
2. Création d'une branche (`git checkout -b feature/ajout-fonctionnalité`)
3. Commit de vos changements (`git commit -am 'Ajout de la fonctionnalité'`)
4. Push vers la branche (`git push origin feature/ajout-fonctionnalité`)
5. Création d'un nouveau Pull Request

## Licence

Ce projet est sous licence MIT. Voir le fichier [LICENSE](LICENSE) pour plus de détails.

## Auteurs

- 👤 **Betzalel75**: [Github](https://github.com/Betzalel75)
- 👤 **Nasser269**: [Github](https://github.com/Nasser269)
- 👤 **DedSlash**: [Github](https://github.com/DedSlash)
---


# Redisk - Clone de Redis avec Persistance sur Disque

Redisk est une implémentation simplifiée de Redis en Rust qui privilégie la persistance sur le disque tout en offrant une couche de cache mémoire (LRU) configurable.

## Fonctionnalités

- **Persistance sur disque** : Toutes les écritures sont enregistrées de manière séquentielle dans un fichier de données. L'index est reconstruit en mémoire au démarrage.
- **Cache LRU** : Un cache en mémoire permet d'accélérer les lectures. La taille du cache est configurable.
- **Protocole RESP** : Support partiel du protocole Redis (commandes de base).

## Commandes Supportées

- `SET key value` : Stocke une valeur.
- `GET key` : Récupère une valeur (vérifie le cache, puis le disque).
- `DEL key [key ...]` : Supprime une ou plusieurs clés.
- `PING` : Vérifie que le serveur est en vie.

## Configuration

Par défaut, le serveur écoute sur `127.0.0.1:6379`.
Le fichier de stockage est `data.redisk`.
La taille du cache est actuellement fixée à 100 entrées dans `src/main.rs`.

## Utilisation

Pour lancer le serveur :
```bash
cargo run
```

Pour tester avec `redis-cli` :
```bash
redis-cli SET ma_cle "ma valeur"
redis-cli GET ma_cle
```

## Architecture

- `src/storage` : Moteur de stockage append-only avec indexation en mémoire.
- `src/cache` : Couche de cache LRU utilisant la crate `lru`.
- `src/protocol` : Parseur et sérialiseur manuel pour le protocole RESP.
- `src/server` : Serveur asynchrone basé sur `tokio`.

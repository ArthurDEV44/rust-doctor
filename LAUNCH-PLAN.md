# rust-doctor Launch Plan

## Contexte

react-doctor (Aiden Bai) a explosé grâce à 63K followers, un backing YC, et un marché React de 25M devs.
rust-doctor part d'un cold start avec un marché Rust de 4M devs — mais occupe un créneau vide (aucun outil unifié de health scoring en Rust).

Le produit est solide. Le problème est 100% distribution.

---

## Étapes par ordre de priorité

### Phase 1 — Préparer le contenu (avant tout lancement)

- [x] **Blog post technique** : "What rust-doctor catches that clippy alone misses" ✅
  - [x] 5 sections avec code concret (hardcoded-secrets, blocking-in-async, block-on-in-async, unwrap, CVEs)
  - [x] Incidents réels sourcés (Turso, GitGuardian 39M, RUSTSEC-2024-0003, CVE-2024-24576)
  - [x] Data points (CodeRabbit 2.74x, Karpathy vibe coding, 78% AI usage)
  - [x] Publié sur rust-doctor.dev/blog/what-rust-doctor-catches-that-clippy-misses
  - [ ] Cross-poster sur dev.to après le Show HN
- [x] **Vidéo terminal** du scan en action ✅
  - [x] MP4 Remotion (725KB) dans `assets/demo.mp4`
  - [x] Intégrée dans le README GitHub
  - [x] Convertir en GIF pour les contextes où la vidéo ne se lit pas (Reddit, etc.) ✅ (147KB dans `assets/demo.gif`)
- [x] **README GitHub** — en anglais ✅ (partiel)
  - [x] One-liner d'install clair (`npx rust-doctor`, `cargo install rust-doctor`)
  - [x] Vidéo du terminal intégrée
  - [x] Badges (crates.io version, npm version, CI status, crates.io downloads, npm downloads)
- [x] **Website** rust-doctor.dev ✅
  - [x] Homepage Next.js avec hero, checks, score, MCP, install, rules, FAQ
  - [x] SEO : 6 schémas JSON-LD, robots.txt, llms.txt, OG image programmatique
  - [x] Dark/light/system theme
  - [x] Optimisation images mascotte (3.6MB → 93-107KB PNG + 17-22KB WebP)
  - [x] Route `/docs` (Fumadocs — 6 pages + 6 pages règles) et `/blog` (listing + articles MDX)

### Phase 2 — Construire un minimum de crédibilité HN (2-3 jours)

- [x] **Compte Hacker News** créé : `arthurjean`
- [x] Upvoter quelques posts techniques ✅ (3 upvotes : Rust WASM parser, Claude Code contractor, FFmpeg Show HN)
- [x] Laisser 1-2 commentaires pertinents sur des threads Rust/tooling/dev ✅ (2 commentaires : Rust WASM parser, piping contractor Claude Code)
- [ ] Accumuler un peu de karma avant le Show HN (en cours — karma à 0, en attente d'upvotes. Show HN prévu mardi 25 mars)

### Phase 3 — Lancement coordonné (même jour)

- [ ] **Show HN** sur Hacker News
  - Titre : `Show HN: Rust Doctor – Unified health scanner for Rust projects (0-100 score)`
  - URL : le blog post (ou GitHub si pas de blog)
  - Poster entre 14h-19h heure française (12-17 UTC)
  - Répondre à TOUS les commentaires rapidement
- [ ] **Post r/rust** sur Reddit
  - Post substantiel (pas juste un lien) — expliquer le problème et la solution
  - Inclure la GIF/screenshot
- [x] **Tweet en anglais** sur X (@arthurstrivex) ✅
  - [x] Vidéo Remotion intégrée
  - [x] https://x.com/arthurstrivex/status/2034583197975986581

### Phase 4 — Distribution passive (jours suivants)

- [ ] **Soumettre à "This Week in Rust"** via leur repo GitHub
  - https://github.com/rust-lang/this-week-in-rust
- [ ] **PR vers awesome-rust** (29K stars)
  - https://github.com/rust-unofficial/awesome-rust
  - Ajouter rust-doctor dans la catégorie appropriée
- [ ] **Post LinkedIn en anglais** (version retravaillée)

### Phase 5 — Amplification (si traction initiale)

- [ ] **Contacter des influenceurs Rust** (DM ou mention)
  - ThePrimeagen — Rust/perf, audience massive
  - Jon Gjengset — éducation Rust, très respecté
  - fasterthanlime — blog Rust populaire
- [ ] **Soumettre à d'autres newsletters / agrégateurs**
  - Rust Magazine
  - r/programming sur Reddit
  - Lobste.rs

---

## Chiffres de référence

- Show HN moyen : **121 stars en 24h, 289 la première semaine**
- Horaire optimal HN : **12-17 UTC**
- react-doctor : 5.8K stars, 1M vues tweet, 84K scans en 24h (mais avec 63K followers pré-existants)

## Avantage compétitif

Aucun outil Rust ne fait ce que rust-doctor fait :
- clippy = lints uniquement
- cargo-audit = sécurité uniquement
- cargo-deny = licences/deps uniquement
- **rust-doctor = tout agrégé en un score 0-100 avec 18 règles AST custom**

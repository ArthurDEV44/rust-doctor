# Rapport Best Practices — rust-doctor

**Date :** 2026-03-17
**Stack :** Rust (edition 2024, MSRV 1.85) / rmcp 1.2.0 + clap 4.6 / Cargo
**Taille du projet :** small (24 fichiers source, ~10 000 LOC)

---

## Resume executif

rust-doctor est un projet de bonne facture avec des fondations solides : architecture modulaire bien decoupee (public API vs. internes), hierarchie d'erreurs typee avec `thiserror`, pipeline de scan parallelise, tests unitaires soignes avec `insta`, et CI fonctionnelle. Les principaux axes d'amelioration sont : (1) les handlers MCP bloquent le runtime tokio pendant 5-30s, (2) `tokio`/`rmcp` sont compiles inconditionnellement meme en mode CLI-only, (3) la CI n'inclut pas `cargo audit` et applique des lints plus faibles que l'outil lui-meme, (4) l'entree MCP `directory` n'est pas contrainte, et (5) la documentation publique est absente. Le code est propre et idiomatique — les ecarts sont corriger-ables sans refactoring majeur.

**Score global :** 62/100

| Categorie | Conformes | Ecarts majeurs | Ecarts mineurs | Absents |
|-----------|-----------|---------------|----------------|---------|
| Architecture | 2 | 0 | 2 | 0 |
| Qualite | 1 | 2 | 3 | 1 |
| Securite | 2 | 1 | 1 | 0 |
| Performance | 0 | 1 | 3 | 1 |
| Tests | 1 | 0 | 3 | 2 |
| Conventions | 2 | 1 | 0 | 1 |
| DX | 1 | 1 | 1 | 1 |
| **Total** | **9** | **6** | **13** | **6** |

---

## Quick Wins (effort faible, impact eleve)

### QW-1: Ajouter `#![forbid(unsafe_code)]` sur le crate root

- **Best practice :** Declarer `#![forbid(unsafe_code)]` dans les crates qui n'utilisent pas d'`unsafe` pour prevenir les regressions
- **Source :** [Rust Security Best Practices 2025 — Corgea](https://corgea.com/Learn/rust-security-best-practices-2025)
- **Ecart :** Aucun `unsafe` n'existe dans le codebase, mais le compilateur ne l'interdit pas — un contributeur pourrait en introduire sans friction
- **Impact :** securite — prevention compile-time contre l'introduction accidentelle d'`unsafe`
- **Fichier(s) :** `src/lib.rs:1`
- **Avant :**
  ```rust
  #![warn(clippy::pedantic)]
  // Expect these pedantic lints project-wide...
  ```
- **Apres :**
  ```rust
  #![forbid(unsafe_code)]
  #![warn(clippy::pedantic)]
  // Expect these pedantic lints project-wide...
  ```

### QW-2: Ajouter `cargo audit` dans le pipeline CI

- **Best practice :** Executer `cargo audit` sur chaque push et en cron hebdomadaire pour detecter les CVE dans les dependances transitives
- **Source :** [Rust Auditing Tools 2025 — Markaicode](https://markaicode.com/rust-auditing-tools-2025-automated-security-scanning/)
- **Ecart :** Le pipeline CI execute `cargo check`, `clippy`, `fmt`, `test`, `publish --dry-run` mais pas `cargo audit`. Le projet depend de 13 crates directes et de dizaines de transitives sans verification automatisee de CVE
- **Impact :** securite — les vulnerabilites dans les dependances transitives ne sont pas detectees avant production
- **Fichier(s) :** `.github/workflows/ci.yml:33`
- **Avant :**
  ```yaml
      - name: Test
        run: cargo test

      - name: Publish dry-run
        run: cargo publish --dry-run
  ```
- **Apres :**
  ```yaml
      - name: Test
        run: cargo test

      - name: Audit
        run: cargo install cargo-audit && cargo audit

      - name: Publish dry-run
        run: cargo publish --dry-run
  ```

### QW-3: Aligner les lints CI avec le niveau que l'outil applique aux projets utilisateurs

- **Best practice :** Le CI d'un outil de qualite de code doit s'appliquer au minimum les memes regles qu'il impose a ses utilisateurs
- **Source :** [Building a Fast and Reliable CI/CD Pipeline for Rust Crates — NashTech](https://blog.nashtechglobal.com/building-a-fast-and-reliable-ci-cd-pipeline-for-rust-crates/)
- **Ecart :** CI execute `cargo clippy -- -D warnings` (warnings par defaut uniquement). L'outil lui-meme lance `-W clippy::all -W clippy::pedantic -W clippy::nursery -W clippy::cargo` sur les projets scannes. rust-doctor pourrait avoir des issues qu'il flaggerait chez ses utilisateurs mais pas dans son propre CI
- **Impact :** conventions — credibilite de l'outil ("physician heal thyself")
- **Fichier(s) :** `.github/workflows/ci.yml:28`
- **Avant :**
  ```yaml
      - name: Clippy
        run: cargo clippy -- -D warnings
  ```
- **Apres :**
  ```yaml
      - name: Clippy
        run: cargo clippy -- -W clippy::all -W clippy::pedantic -W clippy::nursery -D warnings
  ```

### QW-4: Ajouter un cap de taille de fichier avant `syn::parse_file`

- **Best practice :** Limiter les ressources consommees lors de l'analyse de code externe pour prevenir les DoS
- **Source :** [Rust Security Best Practices 2025 — Corgea](https://corgea.com/Learn/rust-security-best-practices-2025)
- **Ecart :** `collect_rs_files_recursive` collecte tous les `.rs` sans limite de taille. `syn::parse_file` parse l'AST entier en memoire. Un fichier `.rs` de 200 MB provoquerait un OOM, surtout en mode MCP ou le `directory` est fourni par un client externe
- **Impact :** securite / performance — prevention OOM en mode MCP
- **Fichier(s) :** `src/scanner.rs:235-262`
- **Avant :**
  ```rust
  if meta.is_file() && path.extension().is_some_and(|e| e == "rs") {
      files.push(path);
  }
  ```
- **Apres :**
  ```rust
  const MAX_RS_FILE_BYTES: u64 = 10 * 1024 * 1024; // 10 MB
  if meta.is_file() && path.extension().is_some_and(|e| e == "rs") {
      if meta.len() <= MAX_RS_FILE_BYTES {
          files.push(path);
      } else {
          eprintln!("Warning: skipping oversized file {} ({} bytes)", path.display(), meta.len());
      }
  }
  ```

### QW-5: Wrapper `scan_project` dans `spawn_blocking` pour les handlers MCP

- **Best practice :** Ne jamais bloquer les worker threads tokio avec du travail CPU-bound ou des I/O synchrones — utiliser `spawn_blocking`
- **Source :** [Best Practices for Tokio: A Comprehensive Guide — OreateAI](https://www.oreateai.com/blog/best-practices-for-tokio-a-comprehensive-guide-to-writing-efficient-asynchronous-rust-code/fab15751330fc07d6632c61da87a5bab)
- **Ecart :** Les handlers MCP `scan` et `score` appellent `scan::scan_project(...)` directement dans `async fn`. Cette fonction lance `std::thread::scope`, `rayon::par_iter`, et des subprocesses bloquants (`cargo clippy`, `cargo audit`). Un worker thread tokio est bloque pendant 5-30 secondes
- **Impact :** performance — le runtime async est monopolise, empechant tout traitement concurrent (progress notifications, autres requetes MCP)
- **Fichier(s) :** `src/mcp.rs:159`, `src/mcp.rs:205`
- **Avant :**
  ```rust
  let result = scan::scan_project(&project_info, &resolved, false, &[], true)
      .map_err(|e| McpError::internal_error(e.to_string(), None))?;
  ```
- **Apres :**
  ```rust
  let result = tokio::task::spawn_blocking(move || {
      scan::scan_project(&project_info, &resolved, false, &[], true)
  })
  .await
  .map_err(|e| McpError::internal_error(format!("scan task panicked: {e}"), None))?
  .map_err(|e| McpError::internal_error(e.to_string(), None))?;
  ```

---

## Ameliorations a moyen terme

### AM-1: Gater les dependances MCP derriere un feature flag Cargo

- **Best practice :** Les dependances lourdes utilisees uniquement par un sous-ensemble de fonctionnalites doivent etre conditionnelles via feature flags
- **Source :** [How to Deal with Rust Dependencies — notgull](https://notgull.net/rust-dependencies/)
- **Ecart :** `tokio` (avec `rt-multi-thread`), `rmcp` (avec `server`, `transport-io`, `macros`), et `schemars` sont toujours compiles. Ils representent ~150+ crates transitives et ne servent qu'au mode `--mcp`. Chaque pipeline CI et chaque utilisateur de la library compile ces dependances inutilement
- **Impact :** DX — temps de compilation significativement plus long pour les utilisateurs CLI-only
- **Fichier(s) :** `Cargo.toml:26-28`
- **Effort estime :** ~2h — restructurer les imports conditionnels avec `#[cfg(feature = "mcp")]`
- **Avant :**
  ```toml
  [dependencies]
  rmcp = { version = "1.2.0", features = ["server", "transport-io", "macros"] }
  tokio = { version = "1.50.0", features = ["rt-multi-thread"] }
  schemars = "1"
  ```
- **Apres :**
  ```toml
  [features]
  default = ["mcp"]
  mcp = ["dep:rmcp", "dep:tokio", "dep:schemars"]

  [dependencies]
  rmcp = { version = "1.2.0", features = ["server", "transport-io", "macros"], optional = true }
  tokio = { version = "1.50.0", features = ["rt-multi-thread"], optional = true }
  schemars = { version = "1", optional = true }
  ```

### AM-2: Valider le parametre `directory` MCP contre un perimetre autorise

- **Best practice :** Valider toute entree externe a la frontiere du systeme — les chemins de fichiers doivent etre contraints
- **Source :** [How to Secure Rust APIs Against Common Vulnerabilities — OneUptime](https://oneuptime.com/blog/post/2026-01-07-rust-api-security/view)
- **Ecart :** Le parametre MCP `directory` accepte n'importe quel chemin absolu. Un client MCP (ou un LLM prompt-injecte) peut pointer vers `/etc`, `/home/other-user`, ou un `Cargo.toml` malveillant contenant un `build.rs` execute par `cargo metadata`. La seule validation est `Path::canonicalize()` qui confirme l'existence sans contraindre le perimetre
- **Impact :** securite — exposition de l'arborescence et potentiel d'execution de code via `build.rs`
- **Fichier(s) :** `src/mcp.rs:141`, `src/discovery.rs:264`
- **Effort estime :** ~1h — ajouter une validation $HOME ou un parametre `allowed_roots`
- **Avant :**
  ```rust
  fn discover_and_resolve(directory: &str) -> Result<(...), McpError> {
      let (target_dir, project_info, file_config) =
          discovery::bootstrap_project(Path::new(directory), false).map_err(|e| { ... })?;
      // ...
  }
  ```
- **Apres :**
  ```rust
  fn discover_and_resolve(directory: &str) -> Result<(...), McpError> {
      let canonical = Path::new(directory).canonicalize()
          .map_err(|e| McpError::invalid_params(format!("invalid path: {e}"), None))?;
      if let Ok(home) = std::env::var("HOME") {
          if !canonical.starts_with(&home) {
              return Err(McpError::invalid_params(
                  "directory must be under $HOME for security", None,
              ));
          }
      }
      let (target_dir, project_info, file_config) =
          discovery::bootstrap_project(&canonical, false).map_err(|e| { ... })?;
      // ...
  }
  ```

### AM-3: Remplacer les `expect()` par `Result` dans le point d'entree MCP

- **Best practice :** Les points d'entree de production ne doivent jamais paniquer — propager les erreurs et les afficher proprement
- **Source :** [Effective Error Handling in Rust CLI Apps — Technorely](https://technorely.com/insights/effective-error-handling-in-rust-cli-apps-best-practices-examples-and-advanced-techniques)
- **Ecart :** `run_mcp_server()` contient trois `.expect()` : creation du runtime tokio, demarrage du serveur, et attente. Un panic dans un serveur MCP longue duree coupe la connexion IDE/agent sans message structure
- **Impact :** fiabilite — un crash non-recoverable la ou une erreur propre serait possible
- **Fichier(s) :** `src/mcp.rs:363-368`
- **Effort estime :** ~30min — changer la signature en `Result` et adapter `main.rs`
- **Avant :**
  ```rust
  pub fn run_mcp_server() {
      let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
      rt.block_on(async {
          let server = RustDoctorServer::new();
          let transport = rmcp::transport::io::stdio();
          let service = server.serve(transport).await.expect("MCP server failed");
          service.waiting().await.expect("MCP server error");
      });
  }
  ```
- **Apres :**
  ```rust
  pub fn run_mcp_server() -> Result<(), Box<dyn std::error::Error>> {
      let rt = tokio::runtime::Runtime::new()?;
      rt.block_on(async {
          let server = RustDoctorServer::new();
          let transport = rmcp::transport::io::stdio();
          let service = server.serve(transport).await?;
          service.waiting().await?;
          Ok(())
      })
  }
  ```

### AM-4: Utiliser `tempfile::TempDir` dans tous les tests au lieu de `std::env::temp_dir().join("fixed-name")`

- **Best practice :** Les tests doivent utiliser des repertoires temporaires uniques qui se nettoient via `Drop`, meme en cas de panic
- **Source :** [The Complete Guide to Rust Testing — Blackwell Systems](https://blog.blackwell-systems.com/posts/rust-testing-comprehensive-guide/)
- **Ecart :** 6+ tests creent des repertoires avec des noms fixes (`rust-doctor-test-no-std`, `rust-doctor-test-config`, etc.) et font un `remove_dir_all` en fin de test. En cas de panic, le nettoyage est saute. `tempfile` est deja en `dev-dependencies` mais non utilise dans ces tests
- **Impact :** tests — stabilite et isolation des tests, surtout en execution parallele
- **Fichier(s) :** `src/discovery.rs:340`, `src/config.rs:421`, `src/suppression.rs:358`, `src/rules/mod.rs:334`
- **Effort estime :** ~1h — remplacer le pattern dans tous les tests concernes
- **Avant :**
  ```rust
  let dir = std::env::temp_dir().join("rust-doctor-test-no-std");
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(dir.join("src")).unwrap();
  // ... test logic ...
  let _ = std::fs::remove_dir_all(&dir);
  ```
- **Apres :**
  ```rust
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join("src")).unwrap();
  // ... test logic ...
  // cleanup automatic on Drop, even on panic
  ```

### AM-5: Ajouter la documentation publique : module-level docs et exemples dans `lib.rs`

- **Best practice :** Chaque item public doit avoir une doc-comment rustdoc. Le crate root doit contenir un exemple executable
- **Source :** [Documentation — Rust API Guidelines](https://rust-lang.github.io/api-guidelines/documentation.html)
- **Ecart :** `lib.rs` n'a aucune documentation module-level. Les `#[expect(clippy::missing_errors_doc)]` et `#[expect(clippy::missing_panics_doc)]` suppriment les lints globalement. Les modules publics (`cli`, `config`, `diagnostics`, `scan`, `output`) manquent de doc-comments
- **Impact :** qualite — les consommateurs de la library API n'ont aucun guide d'usage
- **Fichier(s) :** `src/lib.rs:1-36`
- **Effort estime :** ~2h — ajouter des `//!` et `///` sur les modules et types publics principaux
- **Avant :**
  ```rust
  #![warn(clippy::pedantic)]
  // Expect these pedantic lints...

  // Public API modules
  pub mod cli;
  pub mod scan;
  ```
- **Apres :**
  ```rust
  //! # rust-doctor
  //!
  //! A unified code health tool for Rust — scan, score, and fix your codebase.
  //!
  //! ## Quick Start
  //!
  //! ```no_run
  //! use rust_doctor::{discovery, config, scan};
  //!
  //! let (_, project_info, file_config) = discovery::bootstrap_project(".", false)?;
  //! let resolved = config::resolve_config_defaults(file_config.as_ref());
  //! let result = scan::scan_project(&project_info, &resolved, false, &[], true)?;
  //! println!("Score: {}/100", result.score);
  //! ```

  #![forbid(unsafe_code)]
  #![warn(clippy::pedantic)]

  /// CLI argument parsing and configuration.
  pub mod cli;
  /// Core scanning pipeline.
  pub mod scan;
  ```

---

## Refactors strategiques

### RS-1: Co-localiser les metadonnees de regles avec leur implementation

- **Best practice :** Les donnees qui decrivent une entite (nom, categorie, severite, description, fix) doivent etre co-localisees avec l'implementation de cette entite
- **Source :** [How to Structure a Rust Project Idiomatically — Forem](https://forem.com/sgchris/how-to-structure-a-rust-project-idiomatically-500k)
- **Ecart :** L'ajout d'une regle necessite des modifications dans 3 endroits : l'implementation dans `src/rules/*.rs`, le nom dans `CUSTOM_RULE_NAMES` dans `src/scan.rs:9-29`, et la documentation dans `RULE_DOCS` dans `src/mcp.rs:420-552`. Un test verifie la coherence `RULE_DOCS` → `CUSTOM_RULE_NAMES` mais pas l'inverse. Ce pattern est fragile et freine l'ajout de regles
- **Impact :** maintenabilite — risque eleve d'incoherence lors de l'ajout de nouvelles regles
- **Scope :** `src/rules/mod.rs`, `src/rules/*.rs`, `src/scan.rs`, `src/mcp.rs`
- **Effort estime :** ~4h — enrichir le trait `CustomRule` avec des methodes de metadonnees
- **Approche recommandee :** Ajouter `fn name()`, `fn description()`, `fn fix_hint()`, `fn category()`, `fn severity()` au trait `CustomRule`. Deriver `CUSTOM_RULE_NAMES` et `RULE_DOCS` dynamiquement a partir du registre de regles au lieu de slices statiques paralleles
- **Avant (pattern actuel) :**
  ```rust
  // src/rules/performance.rs
  pub struct ExcessiveClone;
  impl CustomRule for ExcessiveClone {
      fn name(&self) -> &'static str { "excessive-clone" }
      fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> { ... }
  }

  // src/scan.rs — must be manually kept in sync
  pub const CUSTOM_RULE_NAMES: &[&str] = &["excessive-clone", ...];

  // src/mcp.rs — must also be manually kept in sync
  static RULE_DOCS: &[RuleDoc] = &[RuleDoc { name: "excessive-clone", ... }];
  ```
- **Apres (pattern cible) :**
  ```rust
  // src/rules/performance.rs
  pub(crate) struct ExcessiveClone;
  impl CustomRule for ExcessiveClone {
      fn name(&self) -> &'static str { "excessive-clone" }
      fn category(&self) -> Category { Category::Performance }
      fn severity(&self) -> Severity { Severity::Warning }
      fn description(&self) -> &'static str { "Flags .clone() calls..." }
      fn fix_hint(&self) -> &'static str { "Use references or Cow<T>..." }
      fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> { ... }
  }

  // src/scan.rs — derived automatically
  pub fn custom_rule_names(rules: &[Box<dyn CustomRule>]) -> Vec<&str> {
      rules.iter().map(|r| r.name()).collect()
  }
  ```

### RS-2: Cacher le contenu des fichiers source pour eviter les relectures disque

- **Best practice :** Les fichiers analyses ne doivent etre lus qu'une seule fois ; le contenu doit etre partage entre les passes d'analyse
- **Source :** [The Rust Performance Book — Nicholas Nethercote](https://nnethercote.github.io/perf-book/print.html)
- **Ecart :** Chaque fichier `.rs` avec un diagnostic est lu deux fois : une fois par le rule engine (`syn::parse_file`), une fois par `apply_inline_suppressions` dans `src/suppression.rs:42-64`. Pour les gros projets avec beaucoup de diagnostics, c'est un cout I/O redondant mesurable
- **Scope :** `src/rules/mod.rs`, `src/suppression.rs`, `src/scanner.rs`
- **Effort estime :** ~3h — introduire un cache `HashMap<PathBuf, String>` partage entre les passes
- **Approche recommandee :** Lire les fichiers une seule fois dans le rule engine et passer le contenu a la suppression pass, ou utiliser un `Arc<DashMap<PathBuf, String>>` partage
- **Avant (pattern actuel) :**
  ```rust
  // src/suppression.rs:42-64
  for path in unique_paths {
      let abs_path = project_root.join(&path);
      if let Ok(content) = std::fs::read_to_string(&abs_path) {
          // re-reads file already parsed by rule engine
      }
  }
  ```
- **Apres (pattern cible) :**
  ```rust
  // Pass file content cache through the pipeline
  pub fn apply_inline_suppressions(
      diagnostics: Vec<Diagnostic>,
      project_root: &Path,
      file_cache: &HashMap<PathBuf, String>,  // reuse cached content
  ) -> (Vec<Diagnostic>, usize) {
      // use cached content instead of re-reading from disk
  }
  ```

### RS-3: Cacher la disponibilite des outils externes avec `OnceLock`

- **Best practice :** Les verifications couteuses (subprocesses) doivent etre cachees par process lifetime
- **Source :** [Best Practices for Tokio — OreateAI](https://www.oreateai.com/blog/best-practices-for-tokio-a-comprehensive-guide-to-writing-efficient-asynchronous-rust-code/fab15751330fc07d6632c61da87a5bab)
- **Ecart :** `is_clippy_available()`, `is_cargo_audit_available()`, et `is_machete_available()` lancent chacun un subprocess a chaque scan. Pour un workspace a N membres, c'est 3xN subprocesses juste pour les checks de disponibilite (~100-300ms d'overhead)
- **Scope :** `src/clippy.rs:485,499`, `src/audit.rs:22,40`, `src/machete.rs:19,35`
- **Effort estime :** ~1h — remplacer par `OnceLock<bool>`
- **Approche recommandee :** Utiliser `std::sync::OnceLock` pour ne verifier la disponibilite qu'une fois
- **Avant (pattern actuel) :**
  ```rust
  // src/clippy.rs
  fn is_clippy_available() -> bool {
      Command::new("cargo").args(["clippy", "--version"]).output()
          .is_ok_and(|o| o.status.success())
  }
  ```
- **Apres (pattern cible) :**
  ```rust
  use std::sync::OnceLock;

  fn is_clippy_available() -> bool {
      static AVAILABLE: OnceLock<bool> = OnceLock::new();
      *AVAILABLE.get_or_init(|| {
          Command::new("cargo").args(["clippy", "--version"]).output()
              .is_ok_and(|o| o.status.success())
      })
  }
  ```

---

## Findings securite

### HIGH-1: Parametre MCP `directory` sans restriction de perimetre

- **Severite :** HIGH
- **CWE :** CWE-73 (External Control of File Name or Path)
- **Fichier :** `src/mcp.rs:141`, `src/discovery.rs:264`
- **Description :** Le parametre `directory` des outils MCP `scan` et `score` accepte n'importe quel chemin absolu. Un client MCP ou un LLM prompt-injecte peut pointer vers n'importe quel repertoire, declenchant `cargo metadata` sur un `Cargo.toml` potentiellement malveillant
- **Remediation :**
  ```rust
  // Avant (vulnerable)
  let (_dir, project_info, mut resolved) = discover_and_resolve(&input.directory)?;

  // Apres (corrige)
  fn validate_directory(dir: &str) -> Result<PathBuf, McpError> {
      let canonical = Path::new(dir).canonicalize()
          .map_err(|e| McpError::invalid_params(format!("invalid path: {e}"), None))?;
      if let Ok(home) = std::env::var("HOME") {
          if !canonical.starts_with(&home) {
              return Err(McpError::invalid_params(
                  "directory must be under $HOME", None,
              ));
          }
      }
      Ok(canonical)
  }
  ```

### MEDIUM-1: `cargo audit` en mode MCP sans `--no-fetch` — requetes reseau vers des sources non-fiables

- **Severite :** MEDIUM
- **CWE :** CWE-918 (SSRF-adjacent)
- **Fichier :** `src/mcp.rs:159`, `src/audit.rs:55-61`
- **Description :** En mode MCP, `offline` est code en dur a `false`. `cargo audit` effectue un `--fetch` par defaut, telechargement de la base advisory depuis le reseau. Si le `Cargo.lock` du projet pointe vers des sources non-standard, des requetes reseau non prevues sont declenchees
- **Remediation :**
  ```rust
  // Avant
  scan::scan_project(&project_info, &resolved, false, &[], true)

  // Apres — ajouter un parametre `offline` a ScanInput ou defaulter a true en MCP
  scan::scan_project(&project_info, &resolved, true, &[], true) // default offline in MCP
  ```

### MEDIUM-2: Configuration projet (`rust-doctor.toml`) peut desactiver les regles de securite

- **Severite :** MEDIUM
- **CWE :** CWE-732 (Incorrect Permission Assignment)
- **Fichier :** `src/config.rs:56-76`
- **Description :** Un projet malveillant peut inclure un `rust-doctor.toml` avec `ignore.rules = ["hardcoded-secrets", "sql-injection-risk"]` pour desactiver les regles de securite. En mode enforcement (CI gate, MCP audit), cela mine la fiabilite du scan
- **Remediation :**
  ```rust
  // Ajouter un flag --no-project-config / un parametre MCP ignore_project_config
  // Et/ou avertir quand des regles de securite sont supprimees :
  let security_rules = ["hardcoded-secrets", "sql-injection-risk", "unsafe-block-audit"];
  for rule in &resolved.ignore_rules {
      if security_rules.contains(&rule.as_str()) {
          eprintln!("Warning: security rule '{rule}' suppressed by project config");
      }
  }
  ```

### MEDIUM-3: Messages d'erreur MCP exposent des chemins systeme internes

- **Severite :** MEDIUM
- **CWE :** CWE-209 (Information Exposure Through an Error Message)
- **Fichier :** `src/mcp.rs:390-401`, `src/error.rs:19-21`
- **Description :** Les erreurs `cargo_metadata` sont propagees verbatim dans les reponses `McpError`, potentiellement exposant la structure du filesystem du serveur au client MCP
- **Remediation :**
  ```rust
  // Avant
  McpError::invalid_params(format!("{e}{hint}"), None)

  // Apres — separer message client et log interne
  eprintln!("Internal error: {e}"); // log complet cote serveur
  McpError::invalid_params(format!("Project metadata error{hint}"), None) // message client sanitise
  ```

### MEDIUM-4: Absence de limite sur les glob patterns de `ignore.files` en config

- **Severite :** MEDIUM
- **CWE :** CWE-400 (Uncontrolled Resource Consumption)
- **Fichier :** `src/scanner.rs:186-199`
- **Description :** Les patterns glob de `ignore.files` sont acceptes sans limite de nombre ni de longueur. Des patterns pathologiquement complexes pourraient causer un backtracking couteux dans `globset`
- **Remediation :**
  ```rust
  // Ajouter des limites raisonnables dans build_glob_set
  if patterns.len() > 100 {
      eprintln!("Warning: truncating ignore_files to 100 patterns");
      patterns.truncate(100);
  }
  for p in &patterns {
      if p.len() > 256 {
          eprintln!("Warning: ignoring oversized glob pattern: {}", &p[..50]);
          continue;
      }
  }
  ```

---

## Patterns transversaux

1. **Frontiere de confiance MCP sous-estimee.** Plusieurs ecarts (HIGH-1, MEDIUM-1, MEDIUM-2, MEDIUM-3) partagent la meme cause racine : le mode MCP traite les entrees comme si elles venaient d'un utilisateur de confiance, alors qu'un serveur MCP est expose a des clients externes (IDE, agents LLM, orchestrateurs). Recommandation : definir explicitement le modele de menace MCP et ajouter une couche de validation dediee a l'entree de `discover_and_resolve`.

2. **Trois sources de verite pour les regles.** Le nom, l'implementation, et la documentation des regles vivent dans trois fichiers differents (`scan.rs`, `rules/*.rs`, `mcp.rs`). Chaque ajout de regle necessite des modifications synchronisees. Le test de coherence est unidirectionnel. Ce pattern fragile est la source la plus probable de bugs futurs.

3. **Documentation differee mais jamais rattrapee.** Les `#[expect(clippy::missing_errors_doc)]` et `#[expect(clippy::missing_panics_doc)]` portent la mention "deferred until v1.0", mais aucun tracking ni deadline n'existe. La dette s'accumule avec chaque nouvelle fonction publique.

4. **Overhead de subprocess redondant.** Les verifications de disponibilite des outils externes (`cargo clippy --version`, `cargo audit --version`, `cargo machete --version`) sont executees a chaque scan sans cache. Pour un outil concu pour la CI, cet overhead est mesurable et evitable.

5. **Dualite CLI/MCP bien geree architecturalement.** Le pipeline de scan partage est un bon choix. La separation `scan_project()` -> `render_*()` / `ScanOutput` est propre. L'ecart principal est l'absence de `spawn_blocking` dans les handlers MCP — un fix localise, pas un probleme architectural.

---

## Conformites (ce qui est bien fait)

- **Hierarchie d'erreurs typee avec `thiserror`** — `ScanError`, `DiscoveryError`, `BootstrapError`, `McpToolError`, `PassError` sont des enums structures avec `#[source]` et `#[from]` correctement utilises. Les erreurs ne sont pas stringifiees sauf aux variantes `Workspace` et `Diff`.

- **Separation propre public/interne dans `lib.rs`** — Les modules internes (`audit`, `clippy`, `diff`, `machete`, `process`, `rules`, `scanner`, `suppression`, `workspace`) sont `pub(crate)`. L'API publique est restreinte aux modules necessaires.

- **Architecture modulaire par domaine fonctionnel** — `rules/` contient les regles AST, `scan.rs` orchestre le pipeline, `mcp.rs` fournit l'interface MCP, `cli.rs` definit les arguments, `output.rs` gere le rendu. Chaque module a une responsabilite claire.

- **Utilisation de `#[expect]` au lieu de `#[allow]`** — Le projet utilise `#[expect(clippy::...)]` partout, ce qui signifie que le compilateur avertira si une suppression devient obsolete. C'est une pratique avancee et correcte.

- **Sous-processus securises sans interpolation shell** — Toutes les invocations `Command::new("cargo").args([...])` utilisent le passage d'arguments par tableau, pas `sh -c "..."`. Aucune injection de commande possible.

- **Protection anti-symlink dans la collecte de fichiers** — `collect_rs_files_recursive` utilise `symlink_metadata` et ignore les symlinks, prevenant les boucles infinies et les traversees hors perimetre.

- **Sortie subprocesses plafonnee** — `process.rs:56` plafonne stdout a `MAX_OUTPUT_BYTES`, `clippy.rs` plafonne stderr a 4 KB, `diff.rs` plafonne la sortie git. Aucun risque d'OOM par sortie subprocesses.

- **Snapshot testing avec `insta`** — Les tests dans `tests/snapshots.rs` verifient la serialisation JSON des types publics. `cargo insta review` est utilisable pour les revues interactives.

- **CI fonctionnelle avec caching** — `ci.yml` utilise `Swatinem/rust-cache@v2` pour le cache Cargo et execute les gates check → clippy → fmt → test dans l'ordre correct.

- **Profil release optimise** — `Cargo.toml` configure `strip = true`, `lto = true`, `codegen-units = 1`, `opt-level = "z"` pour des binaires minimaux.

- **Validation de chemin dans la suppression** — `suppression.rs:51-55` verifie `canonicalize().starts_with(project_root)` avant de lire les fichiers, empechant la traversee de repertoire.

- **Watchdog de timeout sur les subprocesses** — Les subprocesses longs (`clippy`, `cargo audit`) ont un watchdog avec timeout qui tue le process et libere les ressources.

---

## Backlog (faible priorite)

| ID | Ecart | Impact | Effort | Categorie |
|----|-------|--------|--------|-----------|
| BL-1 | `Severity` manque de `PartialOrd/Ord` — tri manuel par closure | low | trivial | qualite |
| BL-2 | `validate_config` alloue un `Vec<String>` puis le drop sans l'utiliser | low | trivial | qualite |
| BL-3 | `indicatif` toujours compile meme quand le spinner est supprime (`--json`, `--score`) | low | faible | DX |
| BL-4 | `use std::io::Read` importe dans un block scope au lieu du top du fichier | low | trivial | conventions |
| BL-5 | Les tests de disponibilite des outils (`test_clippy_is_available`) n'assertent rien | low | trivial | tests |
| BL-6 | `ScoreLabel::Critical` non couvert par les snapshot tests | low | faible | tests |
| BL-7 | Les diagnostics sont construits inline (32 sites) au lieu d'utiliser `ctx.diagnostic()` | medium | moyen | qualite |
| BL-8 | `build_result` traverse les diagnostics 4 fois au lieu d'une seule passe | low | faible | performance |
| BL-9 | Le clap `env` feature n'est pas active — pas de fallback env-var pour les options CLI | low | faible | DX |
| BL-10 | `proc-macro2` est une dependance directe potentiellement redondante avec `syn` | low | faible | dependencies |

---

## Sources

### Best practices consultees
- [How to Design Error Types with thiserror and anyhow in Rust](https://oneuptime.com/blog/post/2026-01-25-error-types-thiserror-anyhow-rust/view) — hierarchie d'erreurs thiserror/anyhow
- [Rust Error Handling Compared: anyhow vs thiserror vs snafu](https://dev.to/leapcell/rust-error-handling-compared-anyhow-vs-thiserror-vs-snafu-2003) — chaines d'erreurs et #[source]
- [From println!() Disasters to Production: Building MCP Servers in Rust](https://dev.to/ejb503/from-println-disasters-to-production-building-mcp-servers-in-rust-imf) — patterns MCP stdio, error-as-UI, JsonSchema
- [How to Build a Streamable HTTP MCP Server in Rust](https://www.shuttle.dev/blog/2025/10/29/stream-http-mcp) — transports MCP
- [How to Build a CLI Tool in Rust with Clap and Proper Error Handling](https://oneuptime.com/blog/post/2026-01-07-rust-cli-clap-error-handling/view) — clap derive, env feature
- [Building CLI Tools with Rust](https://calmops.com/programming/rust/building-cli-tools-with-rust/) — sortie machine-readable + human-readable
- [Effective Error Handling in Rust CLI Apps](https://technorely.com/insights/effective-error-handling-in-rust-cli-apps-best-practices-examples-and-advanced-techniques) — exit codes POSIX
- [Cargo Workspace Best Practices for Large Rust Projects](https://reintech.io/blog/cargo-workspace-best-practices-large-rust-projects) — organisation par domaine
- [How to Structure a Rust Project Idiomatically](https://forem.com/sgchris/how-to-structure-a-rust-project-idiomatically-500k) — API publique minimale
- [Claude Code Sub-Agents: Parallel vs Sequential Patterns](https://claudefa.st/blog/guide/agents/sub-agent-best-practices) — orchestration multi-agents
- [Claude Code Agent Teams: Best Practices](https://claudefa.st/blog/guide/agents/agent-teams-best-practices) — spawn prompts explicites
- [AI Agent Architecture Patterns for Code Review Automation](https://tanagram.ai/blog/ai-agent-architecture-patterns-for-code-review-automation-the-complete-guide) — analyse statique + LLM
- [Best Practices for Tokio: A Comprehensive Guide](https://www.oreateai.com/blog/best-practices-for-tokio-a-comprehensive-guide-to-writing-efficient-asynchronous-rust-code/fab15751330fc07d6632c61da87a5bab) — spawn_blocking, channels tokio
- [Clippy's Lints — Rust Documentation](https://doc.rust-lang.org/stable/clippy/lints.html) — lint pedantic, suspicious, nursery
- [Setting up Effective CI/CD for Rust — Shuttle](https://www.shuttle.dev/blog/2025/01/23/setup-rust-ci-cd) — CI gates
- [Rust Auditing Tools 2025 — Markaicode](https://markaicode.com/rust-auditing-tools-2025-automated-security-scanning/) — cargo audit, Miri
- [Rust Security Best Practices 2025 — Corgea](https://corgea.com/Learn/rust-security-best-practices-2025) — newtypes, forbid(unsafe_code)
- [How to Secure Rust APIs — OneUptime](https://oneuptime.com/blog/post/2026-01-07-rust-api-security/view) — validation d'entree, OWASP
- [The Rust Performance Book — Nicholas Nethercote](https://nnethercote.github.io/perf-book/print.html) — pre-sizing, allocations
- [The Complete Guide to Rust Testing — Blackwell Systems](https://blog.blackwell-systems.com/posts/rust-testing-comprehensive-guide/) — insta, tests d'integration
- [Rust Testing Libraries — Rustfinity](https://www.rustfinity.com/blog/rust-testing-libraries) — proptest
- [How to Deal with Rust Dependencies — notgull](https://notgull.net/rust-dependencies/) — feature flags, cargo tree
- [Building Flexible and Testable Service Layers with Rust Traits — Leapcell](https://leapcell.io/blog/building-flexible-and-testable-service-layers-with-rust-traits) — dependency inversion
- [Documentation — Rust API Guidelines](https://rust-lang.github.io/api-guidelines/documentation.html) — doc every public item
- [How to Write Documentation — The rustdoc book](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html) — exemples executable
- [SOLID Principles in Rust — 40tude](https://www.40tude.fr/docs/06_programmation/rust/022_solid/solid_00.html) — interface segregation
- [Building a Fast and Reliable CI/CD Pipeline for Rust Crates — NashTech](https://blog.nashtechglobal.com/building-a-fast-and-reliable-ci-cd-pipeline-for-rust-crates/) — fmt → clippy → test → audit

### Audit du codebase
- `src/mcp.rs:159,205` — scan_project bloquant dans async fn
- `src/mcp.rs:363-368` — expect() panics dans le point d'entree MCP
- `src/mcp.rs:141` — directory non-valide en MCP
- `src/error.rs:9-13` — ScanError::Workspace/Diff utilisent String
- `src/config.rs:60-75` — load_file_config swallows parse errors
- `src/scan.rs:283-296` — 4 traversees de diagnostics
- `src/scanner.rs:235-262` — pas de cap de taille de fichier
- `src/suppression.rs:42-64` — relecture des fichiers depuis le disque
- `src/clippy.rs:485,499` — availability check subprocess a chaque scan
- `.github/workflows/ci.yml:28` — lint flags CI plus faibles que l'outil
- `Cargo.toml:26-28` — tokio/rmcp/schemars toujours compiles

---

## Methodologie

Ce rapport a ete genere par le workflow `/meta-best-practices` qui :
1. Recherche les meilleures pratiques actuelles du marche (Anthropic, Google, OpenAI, communaute)
2. Detecte automatiquement le stack du projet
3. Audite le codebase en parallele (qualite + securite)
4. Croise les best practices avec l'etat reel du codebase
5. Classe les ecarts par impact et effort

**Agents utilises :** agent-websearch (x2), agent-explore (x2, qualite + securite)
**Date de recherche :** 2026-03-17

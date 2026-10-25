# Contributing / Contribuindo

🇬🇧 [English](#english) · 🇧🇷 [Português](#português)

## English

Thanks for your interest! Bug reports, ideas and pull requests are welcome, in
English or Portuguese.

- **Issues:** describe what you expected, what happened, your distro, and the
  relevant part of your `config.txt` (remove personal paths if you like).
- **Pull requests:** keep them focused, and make sure these pass before opening:

  ```bash
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test
  ```

- **Code language:** identifiers and comments in `src/` are in Portuguese; keep
  new code consistent with the surrounding code. Comments in English are fine
  too.
- **User-facing text:** every message the program prints must exist in both
  languages. Use `tr!("português", "english")` (formatted) or
  `txt("português", "english")` (plain) from `src/i18n.rs`. Tests always run in
  Portuguese.
- **Config keys:** if you add one, accept both a Portuguese and an English name
  (see `config.rs`) and document it in `config.example.txt`,
  `config.exemplo.txt`, `README.md` and `README.pt-BR.md`.
- **Scope:** the project is intentionally simple: rules only, offline, Linux.

## Português

Obrigado pelo interesse! Relatos de bugs, ideias e pull requests são bem-vindos,
em português ou inglês.

- **Issues:** descreva o que esperava, o que aconteceu, a sua distro e o trecho
  relevante do `config.txt` (pode remover caminhos pessoais).
- **Pull requests:** mantenha-os focados e confira antes de abrir:

  ```bash
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test
  ```

- **Idioma do código:** identificadores e comentários em `src/` estão em
  português; siga o estilo do código ao redor. Comentários em inglês também são
  aceitos.
- **Textos para o usuário:** toda mensagem do programa precisa existir nos dois
  idiomas. Use `tr!("português", "english")` (com formatação) ou
  `txt("português", "english")` (texto simples), de `src/i18n.rs`. Os testes
  sempre rodam em português.
- **Chaves do config:** ao criar uma, aceite um nome em português e um em
  inglês (veja `config.rs`) e documente em `config.example.txt`,
  `config.exemplo.txt`, `README.md` e `README.pt-BR.md`.
- **Escopo:** o projeto é simples de propósito: só regras, offline, Linux.

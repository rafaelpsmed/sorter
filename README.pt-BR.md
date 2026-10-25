# Sorter

🇬🇧 [English](README.md) · 🇧🇷 Português

Monitora uma pasta (ex.: `~/Downloads`) e move cada item que chega para a pasta
da sua categoria, de acordo com regras num arquivo de texto. Também organiza o
que já estava lá quando o programa inicia. Funciona offline, só com regras (sem IA, sem rede).

## Instalação

```bash
./instalar.sh
```

O script compila (precisa do `cargo`), instala o binário em `~/.local/bin`,
cria `~/.config/sorter/config.txt` a partir de `config.exemplo.txt`,
mostra uma simulação e, se você confirmar, ativa o serviço do systemd do
usuário, que inicia junto com a sessão. Para iniciar já no boot, antes do login:
`loginctl enable-linger $USER`.

Desinstalar: `./instalar.sh --desinstalar` (o config é mantido).

### Levar para outro PC

```bash
./empacotar.sh
```

Gera em `dist/` duas opções com o mesmo conteúdo: a pasta `sorter/` e
o arquivo `sorter.tar.gz`. O pacote leva o executável já compilado (o
outro PC não precisa de Rust) e o seu config atual. No outro PC:

```bash
tar xzf sorter.tar.gz   # só se levou o .tar.gz
cd sorter
./instalar.sh
```

Se o outro PC já tiver um config instalado, ele é mantido. O executável roda
em distros com glibc 2.34 ou mais nova (Ubuntu 22.04+, Debian 12+, Fedora 35+).
Para um executável totalmente estático, instale o alvo musl (no Arch:
`pacman -S rust-musl`) antes de rodar o `empacotar.sh`.

Use `~` e `$XDG_..._DIR` no config, nunca `/home/seu-usuario`: assim o mesmo
arquivo funciona com qualquer usuário e em qualquer idioma do sistema.

## Configuração

Veja os comentários em `config.exemplo.txt` (em inglês: `config.example.txt`).
As chaves funcionam em português ou em inglês (`PASTA_FONTE` = `SOURCE_DIR`,
`PASTA_OUTROS` = `OTHERS_DIR`, `ESPERA_SEGUNDOS` = `WAIT_SECONDS`,
`NOTIFICACOES` = `NOTIFICATIONS`, `CATEGORIAS` = `CATEGORIES`, `palavras` =
`words`). Resumo:

```text
PASTA_FONTE: $XDG_DOWNLOAD_DIR
PASTA_OUTROS: $XDG_DOWNLOAD_DIR/OUTROS
ESPERA_SEGUNDOS: 5
NOTIFICACOES: sim
CATEGORIAS:
1. VIDEOS: $XDG_VIDEOS_DIR | ext: mp4, mkv
2. NOTAS_FISCAIS: ~/Documentos/Notas | ext: pdf | palavras: nota fiscal, nfe, recibo
3. LIVROS: ~/Livros | ext: pdf, epub
```

Pastas de destino que não existirem são criadas.

**Mudanças no config são aplicadas assim que o arquivo é salvo**, sem
reiniciar nada:

- as regras novas passam a valer, e a pasta monitorada é reorganizada com elas;
- o que estava em `PASTA_OUTROS` e agora se encaixa numa categoria sai de lá
  (ex.: criou uma categoria para `.json` → os `.json` de OUTROS vão para ela);
- se `PASTA_FONTE` mudar, o programa passa a vigiar a pasta nova;
- se o config salvo tiver erro, a configuração anterior continua valendo e
  aparece uma notificação com a linha do problema. Se o programa já iniciar
  com o config errado, ele espera o arquivo ser corrigido.

Itens que já foram movidos para as pastas das categorias não são mexidos: se
você mudar o destino de uma categoria, só os próximos itens vão para o novo
lugar.

## Como a classificação funciona

**Arquivos soltos** na pasta monitorada:

| regras da categoria | quando casa                        | prioridade |
|---------------------|------------------------------------|------------|
| ext + palavras      | extensão **e** pelo menos uma palavra | 3 (maior) |
| só palavras         | pelo menos uma palavra no nome     | 2          |
| só ext              | extensão                           | 1          |

Empate: ganha quem tiver mais palavras encontradas e, depois, quem vier
primeiro no config. As palavras ignoram maiúsculas e acentos. Palavras de até
3 letras (`nfe`, `rm`) precisam aparecer isoladas, então `anfe.pdf` não conta
como `nfe`. Se a extensão não estiver em nenhuma categoria (ou o arquivo não
tiver extensão), o tipo é detectado pelo conteúdo (ex.: um PDF sem `.pdf`).
Sem regra que case, o arquivo vai para `PASTA_OUTROS`.

**Subpastas** são movidas inteiras, nunca desmontadas. Cada arquivo dentro
delas é classificado (os nomes das pastas também contam como palavras) e a
pasta vai para a categoria de mais da metade dos arquivos. Sem maioria (ex.:
código-fonte extraído), vai para `PASTA_OUTROS`.

**Quando um item é movido:** só depois de ficar `ESPERA_SEGUNDOS` sem mudar
e sem arquivos temporários de download (`.part`, `.crdownload`, ...). Pastas
vazias ficam onde estão. Nada é sobrescrito: se o nome já existir no destino,
o item vira `nome (1).ext`.

**Notificações:** a cada leva de itens movidos aparece uma notificação na
área de trabalho (ex.: "3 itens organizados", com até 5 itens listados). Erros
aparecem primeiro e como urgentes. Usa o `notify-send` (pacote `libnotify`);
sem ele, o programa funciona normalmente, só que sem notificações. Para
desligar: `NOTIFICACOES: nao`.

**Ignorados:** itens ocultos (começando com `.`), o próprio config, as pastas
de destino que ficam dentro da pasta monitorada e links simbólicos.

## Uso manual

```bash
sorter --simular   # mostra o que faria, sem mover nada (--dry-run)
sorter --uma-vez   # organiza o que já existe e sai (--once)
sorter -c outro-config.txt
journalctl --user -u sorter -f   # acompanhar o serviço
```

## Idioma

As mensagens do programa (terminal e notificações) seguem o idioma do sistema:
português se `LC_ALL`, `LC_MESSAGES` ou `LANG` começar com `pt`, inglês nos
demais casos. Os scripts `instalar.sh` e `empacotar.sh` fazem o mesmo.

## Desenvolvimento

```bash
cargo test
cargo build --release
```

Código em `src/`: `config.rs` (leitura do config), `classificador.rs`
(regras), `mover.rs` (mover sem sobrescrever, inclusive entre discos),
`texto.rs` (normalização de nomes), `notificacao.rs` (avisos na área de
trabalho), `i18n.rs` (idioma das mensagens) e `main.rs` (monitoramento).

O código-fonte e os comentários estão em português. Veja o
[CONTRIBUTING.md](CONTRIBUTING.md) para contribuir.

## Licença

[MIT](LICENSE)

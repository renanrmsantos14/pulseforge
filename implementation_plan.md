# PulseForge — plano técnico revisado

## Stack recomendada

**Rust + GTK4 + libadwaita + PipeWire**. Melhor para utilitário Linux residente:
binário leve, baixo consumo, integração nativa, acessibilidade e autostart.

- Rust: DSP, threads e segurança de memória.
- GTK4/libadwaita: UI nativa, tema, scaling e teclado.
- PipeWire + WirePlumber: sinks, nós, eventos e troca automática.
- DSP próprio em Rust: biquad 10/15/31 bandas, preamp e limiter opcional.
- JSON versionado em XDG config: simples, exportável, sem banco.
- PulseAudio: compatibilidade via PipeWire; fallback explícito se necessário.

Tauri/React fica como plano B: adiciona WebView e memória sem vantagem real
para app desktop Linux pequeno.

## Arquitetura

```text
GTK UI ── canais async ── Audio Service ── PipeWire Graph
   │             │                 ├── sink detector
   │             │                 ├── profile matcher
   │             │                 └── DSP filter node
   └─────────────┴── Config Store + Autostart + diagnostics
```

UI nunca bloqueia áudio; thread de áudio não aloca; sliders usam debounce;
perfil ativo vira snapshot imutável.

## Fases

### 0. Spike de ambiente

Detectar Mint, PipeWire, WirePlumber e GTK4. Enumerar sinks, identificar sink
padrão e testar criação de node de filtro antes de construir UI.

### 1. Motor de áudio

Criar `AudioBackend` e implementação PipeWire. Implementar cadeia biquad,
bypass, preamp, limiter opcional e atualização sem glitches. Medir CPU,
memória e latência.

### 2. Perfis

Modelo versionado com bandas, preamp, limiter e regras. Presets Flat, Bass
Boost, Vocal, Night e Headphones. CRUD, duplicar, importar/exportar e backup
automático antes de migração.

### 3. Saídas automáticas

Match por identificador persistente, nome, tipo e fallback global. Aplicar perfil
quando sink conecta/desconecta ou vira padrão. Mostrar motivo da seleção e
permitir desligar automação por dispositivo.

### 4. UI

Dashboard com perfil, saída, bypass, preamp e status. Equalizador 10/15/31
bandas com teclado. Espectro GTK a 30–60 FPS. Telas de perfis, regras e
diagnóstico de backend/node/latência/CPU.

### 5. Desktop

Autostart via `.desktop`, opção iniciar minimizado, tray opcional, notificações
somente em erro/troca relevante. Tema escuro instrument-panel: grafite, ciano
elétrico e âmbar. Motion 150–300 ms, sem reflow, reduzido quando solicitado.

### 6. Entrega

Build Cargo, pacote `.deb`, AppImage opcional, ícone SVG, desinstalação limpa e
README de troubleshooting.

## Arquivos previstos

- `[NEW]` `Cargo.toml`, `src/main.rs`, `src/app.rs`
- `[NEW]` `src/audio/{backend,pipewire,dsp}.rs`
- `[NEW]` `src/config/{model,store}.rs`
- `[NEW]` `src/{profiles,output_rules,autostart}.rs`
- `[NEW]` `src/ui/{dashboard,equalizer,profiles,rules,diagnostics}.rs`
- `[NEW]` `data/pulseforge.desktop`, `data/icons/`, `tests/`, `README.md`

## Critérios de aceite

- Inicialização até 1 s e alvo abaixo de ~80 MB RAM.
- Sem glitch ao alterar ganho rapidamente.
- Perfil correto aplicado em até 500 ms após mudança de saída.
- Configuração sobrevive reinício e atualização.
- Operável por mouse, teclado e leitor de tela.
- Autostart configurável no app.
- `cargo test`, `cargo clippy -- -D warnings` e build `.deb` passam.

## Riscos

- Validar DSP no graph PipeWire antes da UI.
- Tray GTK4 é opcional, nunca requisito central.
- Limitar espectro e evitar alocações na thread de áudio.

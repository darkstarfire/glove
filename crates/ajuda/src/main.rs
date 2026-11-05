//! Ajuda do Glove (como o Visualizador de Ajuda da era): os tópicos à
//! esquerda, com busca, e a página à direita. O texto mora aqui mesmo.

use glove_kit::{
    paint::{draw::*, Color},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions, SEARCH_H,
};

/// (título, texto). No texto: "# " é subtítulo e "- " é item de lista.
const PAGES: [(&str, &str); 11] = [
    ("Bem-vindo ao Glove", "O Glove é um ambiente de trabalho leve, feito para caber em computadores com pouca memória, com o jeito do Mac OS X da era do Tiger.\n# O essencial\n- A barra de menu fica no alto: à esquerda o menu do Glove (a bolinha) e os menus do app em uso; à direita o Wi-Fi, o som, a bateria, o relógio e a Busca Rápida.\n- O Dock fica embaixo: clique num ícone para abrir o app; o triângulo mostra os que estão abertos.\n- A Mesa mostra o Disco e os pendrives conectados.\n# Onde mudar as coisas\n- Abra as Preferências pelo Dock ou pelo menu do Glove. Use a busca no canto delas para achar um ajuste."),
    ("Janelas", "Cada janela tem os três botões no canto: vermelho fecha, amarelo minimiza para o Dock e verde faz o zoom.\n# Atalhos\n- Super+Q fecha a janela.\n- Super+M minimiza.\n- Super+Tab troca de app; segure o Super e aperte Tab de novo para andar na lista.\n- Super+H oculta o app; Super+Alt+H oculta os outros.\n# Exposé\n- F9 mostra todas as janelas lado a lado.\n- F10 mostra só as do app em uso.\n- F11 afasta as janelas para mostrar a Mesa.\n- Os cantos da tela podem fazer isso também: Preferências ▸ Mesa e Protetor de Tela."),
    ("O Dock", "O Dock guarda os apps que você mais usa e mostra os que estão abertos.\n# Dicas\n- Arraste um app aberto para o Dock, ou use o menu de contexto dele (clique com o botão direito) ▸ Manter no Dock.\n- Janelas minimizadas aparecem como miniaturas à direita.\n- A Lixeira fica na ponta direita; clique para ver o que está nela.\n- Tamanho, ampliação, posição e ocultar sozinho: Preferências ▸ Dock."),
    ("Arquivos", "O Arquivos mostra suas pastas como ícones, lista ou colunas.\n# Coisas que dá para fazer\n- Arraste para mover; segure Ctrl ao soltar para copiar.\n- Ctrl+C e Ctrl+V copiam e colam; Ctrl+X recorta.\n- Ctrl+Backspace manda para a Lixeira; Devolver traz de volta.\n- Clique com o botão direito para o menu: Obter Informações, Duplicar, Criar Atalho, Compactar.\n- A busca da barra cria uma pasta de resultados; Salvar a guarda na barra lateral.\n- Clique duplo num .zip expande o arquivo ao lado dele."),
    ("Busca Rápida", "Aperte Super+espaço (ou clique na lupa no canto da barra de menu) e comece a digitar.\n# O que ela acha\n- Aplicativos, painéis das Preferências e arquivos da sua pasta pessoal.\n- Use as setas para escolher e Enter para abrir."),
    ("Atalhos de teclado", "Os atalhos principais (todos podem ser mudados em Preferências ▸ Teclado ▸ Atalhos de Teclado):\n- Super+Enter abre o Terminal.\n- Super+vírgula abre as Preferências.\n- Super+espaço abre a Busca Rápida.\n- Super+L bloqueia a tela.\n- Ctrl+Alt+Esc abre Forçar Encerrar.\n- Print captura a tela; Shift+Print captura uma área.\n- Super+Alt+espaço troca o layout do teclado.\n- Super+Alt+8 liga o zoom; Ctrl+Super+Alt+8 inverte as cores."),
    ("Correio", "O Correio lê e envia e-mails pela sua conta (IMAP e SMTP, com conexão segura).\n# Começar\n- Na primeira vez, a janela Contas de Correio pede nome, e-mail e senha; os servidores dos provedores comuns (Gmail, Outlook, Yahoo, UOL, Terra) são preenchidos sozinhos.\n- No Gmail e em outros que pedem, use uma senha de app, criada na página de segurança da sua conta.\n# No dia a dia\n- Receber (Ctrl+Shift+N) busca mensagens novas; o Correio também confere sozinho a cada 5 minutos.\n- Responder (Ctrl+R), Responder a Todos (Ctrl+Shift+R) e Encaminhar (Ctrl+Shift+F) abrem uma janela de escrever; Enviar é Ctrl+Shift+D.\n- Apagar (Delete) manda a mensagem para o Lixo; os anexos têm o botão Salvar, que guarda na pasta Downloads.\n- As mensagens já abertas ficam guardadas no computador e podem ser lidas sem rede."),
    ("Capturas de tela", "As capturas vão para a sua pasta Imagens.\n# Jeitos de capturar\n- Print: a tela inteira.\n- Shift+Print: arraste para escolher a área; Esc cancela.\n- O app Captura deixa escolher a tela inteira, uma área e um atraso de alguns segundos, e abre a foto no Visualizador."),
    ("Som, rede e energia", "Os ícones da barra de menu dão acesso rápido:\n- O alto-falante mostra o controle de volume; as teclas de volume do teclado também funcionam.\n- O leque do Wi-Fi lista as redes; as que pedem senha abrem as Preferências pedindo a senha.\n- A bateria mostra a carga e quanto tempo falta.\n# Mais ajustes\n- Preferências ▸ Som, Rede e Economizador de Energia."),
    ("Sair, bloquear e desligar", "Pelo menu do Glove (a bolinha no canto):\n- Bloquear Tela pede a sua senha para voltar.\n- Repouso deixa o computador dormindo.\n- Reiniciar, Desligar e Encerrar Sessão pedem confirmação e fecham os apps antes; um app com documento não salvo pode cancelar o encerramento."),
    ("Problemas comuns", "# Um app parou de responder\n- O cursor vira o disco girando sobre ele. Use Forçar Encerrar (Ctrl+Alt+Esc) para fechá-lo.\n# Sem som\n- Veja se o som não está mudo no ícone da barra e em Preferências ▸ Som ▸ Saída.\n# Sem rede\n- Abra Preferências ▸ Rede e veja a luz de cada interface; Wi-Fi desligado aparece em vermelho.\n# Ver o que aconteceu\n- O app Registros mostra o registro da sessão do Glove e do sistema."),
];

/// Quebra o texto em linhas que caibam em `max` (palavra por palavra).
fn wrap(ui: &Ui, text: &str, px: f32, bold: bool, max: f32) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let try_line = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if ui.text_width(&try_line, px, bold) > max && !line.is_empty() {
            out.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = try_line;
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

struct Ajuda {
    page: Option<usize>,
    query: String,
    scroll: f32,
}

impl App for Ajuda {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        if ui.keys().iter().any(|k| k.combo == "Ctrl+W") || ui.menu_actions().iter().any(|a| a == "close") {
            ui.close = true;
        }
        // Barra com a busca.
        let bar = 40.0;
        ui.c.fill_rect(0.0, 0.0, w, bar, &vgrad(0.0, bar, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(206, 206, 210, 255))]));
        ui.c.hline(0.0, bar - 1.0, w, rgba(140, 140, 146, 255));
        ui.fonts.draw(ui.c, "Ajuda do Glove", 14.0, bar / 2.0 + 5.0, 13.0, true, rgba(30, 30, 34, 255));
        ui.search_field_in(Rect::new(w - 230.0, (bar - SEARCH_H) / 2.0, 216.0, SEARCH_H), &mut self.query, "Buscar na ajuda");
        // Tópicos (só os que têm as palavras da busca).
        let q = self.query.trim().to_lowercase();
        let found: Vec<usize> = (0..PAGES.len()).filter(|i| q.is_empty() || q.split_whitespace().all(|w| PAGES[*i].0.to_lowercase().contains(w) || PAGES[*i].1.to_lowercase().contains(w))).collect();
        let names: Vec<String> = found.iter().map(|i| PAGES[*i].0.to_string()).collect();
        let mut sel = self.page.and_then(|p| found.iter().position(|i| *i == p));
        ui.area(Rect::new(8.0, bar + 8.0, 190.0, h - bar - 16.0), |ui| {
            ui.list(&mut sel, &names, 190.0, h - bar - 16.0);
        });
        if let Some(i) = sel.and_then(|s| found.get(s)) {
            if self.page != Some(*i) {
                self.page = Some(*i);
                self.scroll = 0.0;
            }
        }
        // A página.
        let r = Rect::new(206.0, bar + 8.0, w - 214.0, h - bar - 16.0);
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE));
        ui.c.stroke(rrect(r.x - 0.5, r.y - 0.5, r.w + 1.0, r.h + 1.0, 0.0), &solid(rgba(150, 150, 156, 255)), 1.0);
        let Some(p) = self.page.filter(|p| found.contains(p)).or_else(|| found.first().copied()) else {
            ui.fonts.draw(ui.c, "Nada encontrado para essa busca.", r.x + 20.0, r.y + 30.0, 12.0, false, rgba(130, 130, 136, 255));
            return;
        };
        let (title, body) = PAGES[p];
        let max = r.w - 50.0;
        let mut lines: Vec<(String, f32, bool, f32)> = vec![(title.to_string(), 18.0, true, 0.0)];
        for para in body.split('\n') {
            if let Some(h2) = para.strip_prefix("# ") {
                lines.push((String::new(), 6.0, false, 0.0));
                lines.push((h2.to_string(), 14.0, true, 0.0));
            } else if let Some(item) = para.strip_prefix("- ") {
                for (k, l) in wrap(ui, item, 12.0, false, max - 16.0).into_iter().enumerate() {
                    lines.push((if k == 0 { format!("•  {l}") } else { format!("    {l}") }, 12.0, false, 6.0));
                }
            } else {
                lines.extend(wrap(ui, para, 12.0, false, max).into_iter().map(|l| (l, 12.0, false, 0.0)));
            }
        }
        let total: f32 = lines.iter().map(|l| l.1 * 1.6).sum::<f32>() + 30.0;
        self.scroll = (self.scroll - ui.scroll_in(r)).clamp(0.0, (total - r.h).max(0.0));
        let mut y = r.y + 30.0 - self.scroll;
        for (t, px, bold, indent) in lines {
            if y > r.y + 10.0 && y < r.y + r.h {
                let ink = if bold { rgba(20, 40, 90, 255) } else { rgba(30, 30, 34, 255) };
                ui.fonts.draw(ui.c, &t, r.x + 22.0 + indent, y, px, bold, ink);
            }
            y += px * 1.6;
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new("Arquivo", vec![MenuItem::new("close", "Fechar", "Ctrl+W")])]
    }
}

fn main() {
    // "glove-ajuda tópico" abre direto num tópico (pela primeira palavra do título).
    let want = std::env::args().nth(1).map(|a| a.to_lowercase());
    let page = want.and_then(|w| PAGES.iter().position(|(t, _)| t.to_lowercase().contains(&w))).or(Some(0));
    let opts = WindowOptions { title: "Ajuda".into(), app_id: "glove.Ajuda".into(), size: (760, 500), fixed: false, min_size: None };
    if let Err(e) = run(opts, Ajuda { page, query: String::new(), scroll: 0.0 }) {
        eprintln!("glove-ajuda: {e}");
    }
}

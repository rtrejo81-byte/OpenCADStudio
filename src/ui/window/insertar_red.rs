//! Ventana de selección de bloque de `INSERTAR_RED`.
//!
//! Sustituye al DCL `POSTES_DIALOG` de la rutina LISP: la misma tabla de 25
//! bloques, en los mismos cuatro grupos. Los bloques que no están definidos en
//! el plano aparecen deshabilitados en vez de fallar al insertarlos.

use crate::app::commands::insertar_red::BLOCKS;
use crate::app::Message;
use iced::{
    widget::{button, column, container, row, scrollable, text},
    Element, Fill,
};

/// Grupos del diálogo original: (título, índices dentro de `BLOCKS`).
const GROUPS: [(&str, &[usize]); 4] = [
    ("Postes de hormigón", &[0, 1, 2, 3, 9, 10, 11, 12]),
    ("Postes metálicos", &[4, 5, 6, 7]),
    ("Otros postes", &[8, 13]),
    (
        "Nuevos bloques",
        &[14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24],
    ),
];

fn exists(available: &[String], name: &str) -> bool {
    available
        .iter()
        .any(|block| block.eq_ignore_ascii_case(name))
}

/// Una tarjeta-botón por bloque; sin `on_press` iced la deja deshabilitada.
fn block_card(index: usize, available: &[String]) -> Element<'static, Message> {
    let (name, label) = BLOCKS[index];
    let present = exists(available, name);
    let body = column![
        text(format!("{}. {}", index + 1, name)).size(13),
        text(if present { label } else { "no está en el plano" }).size(10),
    ]
    .spacing(1)
    .width(Fill);
    let card = button(body).width(Fill).padding(6);
    if present {
        card.on_press(Message::InsertarRedChoose(index as u8))
            .style(button::primary)
            .into()
    } else {
        card.style(button::text).into()
    }
}

pub(crate) fn view_window(available: &[String]) -> Element<'static, Message> {
    let mut groups = column![].spacing(10);
    for (title, indexes) in GROUPS {
        let mut grid = column![text(title).size(14)].spacing(4);
        for chunk in indexes.chunks(2) {
            let mut line = row![].spacing(6);
            for index in chunk {
                line = line.push(block_card(*index, available));
            }
            grid = grid.push(line);
        }
        groups = groups.push(
            container(grid)
                .padding(8)
                .style(container::bordered_box)
                .width(Fill),
        );
    }

    let list = scrollable(groups).height(420).width(Fill);
    let footer = row![
        text("Powered By Trejo · v4.0").size(11),
        iced::widget::Space::new().width(Fill),
        button(text("Terminar en bloque existente"))
            .on_press(Message::InsertarRedChoose(99))
            .style(button::primary),
        button(text("Cancelar")).on_press(Message::CloseModal),
    ]
    .spacing(8)
    .align_y(iced::Center);

    container(column![list, footer].spacing(12))
        .padding(18)
        .width(720)
        .into()
}

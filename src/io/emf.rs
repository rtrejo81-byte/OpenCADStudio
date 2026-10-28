//! Generador mínimo de metarchivos EMF (Enhanced Metafile) para incrustar una
//! imagen en un objeto OLE que AutoCAD y nanoCAD dibujan.
//!
//! Un objeto OLE "Picture (Enhanced Metafile)" guarda un EMF en su flujo
//! CONTENTS. El encabezado de 108 bytes se usa como plantilla literal, copiada
//! de un archivo escrito por nanoCAD que ambas aplicaciones dibujan: solo se
//! parchean dimensiones y tamaños. Ver docs/ole2-hallazgos.md.

/// Firma del encabezado EMF (" EMF").
const EMF_SIGNATURE_OFFSET: usize = 40;
const EMR_HEADER: u32 = 1;
const EMR_STRETCHDIBITS: u32 = 81;
const EMR_EOF: u32 = 14;
/// ROP: copiar el bitmap tal cual.
const SRCCOPY: u32 = 0x00CC_0020;
/// Tamaño del encabezado extendido que usan AutoCAD y nanoCAD.
const TAM_CABECERA: usize = 108;

/// Encabezado de 108 bytes copiado de un EMF que ambos CAD dibujan.
const PLANTILLA: [u8; TAM_CABECERA] = [
    1, 0, 0, 0, 108, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 213, 5, 0, 0, 131, 4, 0, 0, 248, 255, 255, 255, 248, 255, 255, 255, 13, 87, 0, 0, 115, 67, 0, 0, 32, 69, 77, 70, 0, 0, 1, 0, 132, 188, 7, 0, 14, 28, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 0, 0, 160, 5, 0, 0, 126, 1, 0, 0, 215, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 48, 212, 5, 0, 216, 71, 3, 0
];

/// Convierte píxeles RGBA en un EMF que los dibuja a width × height píxeles.
pub fn emf_from_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let ancho = width.max(1);
    let alto = height.max(1);
    // DIB de 32 bits (B,G,R,X) de abajo hacia arriba: cada fila se invierte.
    let mut bits = Vec::with_capacity((ancho * alto * 4) as usize);
    for y in 0..alto {
        let fila = alto - 1 - y;
        for x in 0..ancho {
            let i = (((fila * ancho) + x) * 4) as usize;
            let (r, g, b) = if i + 3 < rgba.len() {
                (rgba[i], rgba[i + 1], rgba[i + 2])
            } else {
                (255, 255, 255)
            };
            bits.extend_from_slice(&[b, g, r, 0]);
        }
    }

    // BITMAPINFOHEADER con alto positivo (filas de abajo hacia arriba).
    let mut bmi = Vec::with_capacity(40);
    bmi.extend_from_slice(&40u32.to_le_bytes());
    bmi.extend_from_slice(&(ancho as i32).to_le_bytes());
    bmi.extend_from_slice(&(alto as i32).to_le_bytes());
    bmi.extend_from_slice(&1u16.to_le_bytes());
    bmi.extend_from_slice(&32u16.to_le_bytes());
    bmi.extend_from_slice(&0u32.to_le_bytes());
    bmi.extend_from_slice(&((ancho * alto * 4) as u32).to_le_bytes());
    bmi.extend_from_slice(&[0u8; 16]);

    // EMR_STRETCHDIBITS: 80 bytes de cabecera + BMI + bits.
    let tam_registro = 80 + bmi.len() + bits.len();
    let mut dib = Vec::with_capacity(tam_registro);
    dib.extend_from_slice(&EMR_STRETCHDIBITS.to_le_bytes());
    dib.extend_from_slice(&(tam_registro as u32).to_le_bytes());
    for valor in [0i32, 0, ancho as i32, alto as i32] {
        dib.extend_from_slice(&valor.to_le_bytes());
    }
    // Destino dentro del marco de la plantilla, conservando la proporcion.
    let marco_ancho = 1493i32;
    let marco_alto = 1155i32;
    let escala = (marco_ancho as f64 / ancho as f64).min(marco_alto as f64 / alto as f64);
    let cx_dest = ((ancho as f64) * escala) as i32;
    let cy_dest = ((alto as f64) * escala) as i32;
    for valor in [0i32, marco_alto - cy_dest, 0, 0, ancho as i32, alto as i32] {
        dib.extend_from_slice(&valor.to_le_bytes());
    }
    dib.extend_from_slice(&80u32.to_le_bytes());
    dib.extend_from_slice(&(bmi.len() as u32).to_le_bytes());
    dib.extend_from_slice(&(80 + bmi.len() as u32).to_le_bytes());
    dib.extend_from_slice(&(bits.len() as u32).to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&SRCCOPY.to_le_bytes());
    dib.extend_from_slice(&cx_dest.to_le_bytes());
    dib.extend_from_slice(&cy_dest.to_le_bytes());
    dib.extend_from_slice(&bmi);
    dib.extend_from_slice(&bits);

    // EMR_EOF
    let mut eof = Vec::with_capacity(20);
    eof.extend_from_slice(&EMR_EOF.to_le_bytes());
    eof.extend_from_slice(&20u32.to_le_bytes());
    eof.extend_from_slice(&0u32.to_le_bytes());
    eof.extend_from_slice(&16u32.to_le_bytes());
    eof.extend_from_slice(&20u32.to_le_bytes());

    let total = TAM_CABECERA + dib.len() + eof.len();
    let mut cabecera = PLANTILLA;
    // Solo se parchean el tamaño total y el número de registros: el resto del
    // encabezado se conserva tal cual porque es el que ambos CAD aceptan.
    cabecera[48..52].copy_from_slice(&(total as u32).to_le_bytes());
    cabecera[52..56].copy_from_slice(&3u32.to_le_bytes());
    debug_assert_eq!(&cabecera[40..44], b" EMF");
    let mut salida = Vec::with_capacity(total);
    salida.extend_from_slice(&cabecera);
    salida.extend_from_slice(&dib);
    salida.extend_from_slice(&eof);
    salida
}
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
pub fn emf_manual(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let ancho = width.max(1);
    let alto = height.max(1);
    // DIB de 32 bits (B,G,R,X) de abajo hacia arriba: cada fila se invierte.
    // 24 bits (B,G,R) sin canal alfa: es el formato que cualquier contexto de
    // dispositivo puede dibujar, incluidos los monocromos de impresión.
    let ancho_fila = ((ancho * 3 + 3) / 4) * 4;
    let mut bits = vec![0u8; (ancho_fila * alto) as usize];
    for y in 0..alto {
        let fila = alto - 1 - y;
        for x in 0..ancho {
            let i = (((fila * ancho) + x) * 4) as usize;
            let (r, g, b) = if i + 3 < rgba.len() {
                (rgba[i], rgba[i + 1], rgba[i + 2])
            } else {
                (255, 255, 255)
            };
            let destino = (y * ancho_fila + x * 3) as usize;
            bits[destino] = b;
            bits[destino + 1] = g;
            bits[destino + 2] = r;
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

/// En Windows el EMF lo genera **GDI**, que es lo que hacen AutoCAD y nanoCAD:
/// un metarchivo grabado por el propio sistema siempre lo reproduce su servidor
/// OLE. El generador manual queda como respaldo en otras plataformas.
#[cfg(target_os = "windows")]
pub fn emf_from_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        CloseEnhMetaFile, CreateCompatibleDC, CreateEnhMetaFileW, DeleteDC, DeleteEnhMetaFile,
        GetEnhMetaFileBits, SetMapMode, SetViewportExtEx, SetWindowExtEx, StretchDIBits,
        BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, MM_ANISOTROPIC, SRCCOPY,
    };

    let ancho = width.max(1);
    let alto = height.max(1);
    // Bits en B,G,R,X de abajo hacia arriba.
    // 24 bits (B,G,R) sin canal alfa: es el formato que cualquier contexto de
    // dispositivo puede dibujar, incluidos los monocromos de impresión.
    let ancho_fila = ((ancho * 3 + 3) / 4) * 4;
    let mut bits = vec![0u8; (ancho_fila * alto) as usize];
    for y in 0..alto {
        let fila = alto - 1 - y;
        for x in 0..ancho {
            let i = (((fila * ancho) + x) * 4) as usize;
            let (r, g, b) = if i + 3 < rgba.len() {
                (rgba[i], rgba[i + 1], rgba[i + 2])
            } else {
                (255, 255, 255)
            };
            let destino = (y * ancho_fila + x * 3) as usize;
            bits[destino] = b;
            bits[destino + 1] = g;
            bits[destino + 2] = r;
        }
    }

    unsafe {
        let referencia = CreateCompatibleDC(std::ptr::null_mut());
        let marco = RECT {
            left: 0,
            top: 0,
            right: (ancho as f64 * 26.4583) as i32,
            bottom: (alto as f64 * 26.4583) as i32,
        };
        let descripcion: Vec<u16> = "lgbcadstudio\0".encode_utf16().collect();
        let hdc = CreateEnhMetaFileW(referencia, std::ptr::null(), &marco, descripcion.as_ptr());
        if hdc.is_null() {
            if !referencia.is_null() {
                DeleteDC(referencia);
            }
            return Vec::new();
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: ancho as i32,
                biHeight: alto as i32,
                biPlanes: 1,
                biBitCount: 24,
                biCompression: 0,
                biSizeImage: ancho_fila * alto,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default(); 1],
        };
        // Registros que los metarchivos de los CAD suelen incluir: modo de mapa
        // y extensión de la ventana. Sin ellos algunos reproductores OLE no
        // escalan el contenido y el dibujo queda fuera del marco.
        SetMapMode(hdc, MM_ANISOTROPIC);
        SetWindowExtEx(hdc, ancho as i32, alto as i32, std::ptr::null_mut());
        SetViewportExtEx(hdc, ancho as i32, alto as i32, std::ptr::null_mut());
        StretchDIBits(
            hdc,
            0,
            0,
            ancho as i32,
            alto as i32,
            0,
            0,
            ancho as i32,
            alto as i32,
            bits.as_ptr() as *const core::ffi::c_void,
            &info,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        let metarchivo = CloseEnhMetaFile(hdc);
        if !referencia.is_null() {
            DeleteDC(referencia);
        }
        if metarchivo.is_null() {
            return Vec::new();
        }
        let tamano = GetEnhMetaFileBits(metarchivo, 0, std::ptr::null_mut());
        let mut salida = vec![0u8; tamano as usize];
        GetEnhMetaFileBits(metarchivo, tamano, salida.as_mut_ptr());
        DeleteEnhMetaFile(metarchivo);
        salida
    }
}

/// Variante: misma plantilla de encabezado pero con las dimensiones del
/// dispositivo, el marco y los milímetros ajustados a la imagen. Sirve para
/// descartar que AutoCAD recorte el dibujo por un tamaño de dispositivo ajeno.
pub fn emf_escala(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let base = emf_manual(width, height, rgba);
    if base.len() < TAM_CABECERA {
        return base;
    }
    let ancho = width.max(1);
    let alto = height.max(1);
    let mm_ancho = ((ancho as f64) * 0.264_583) as u32;
    let mm_alto = ((alto as f64) * 0.264_583) as u32;
    let centi_mm_ancho = ((ancho as f64) * 26.4583) as u32;
    let centi_mm_alto = ((alto as f64) * 26.4583) as u32;
    let mut salida = base;
    let total = salida.len() as u32;
    salida[40..44].copy_from_slice(b" EMF");
    let mut poner = |desde: usize, valor: u32| {
        salida[desde..desde + 4].copy_from_slice(&valor.to_le_bytes());
    };
    poner(0, 1);
    poner(4, TAM_CABECERA as u32);
    poner(8, 0);
    poner(12, 0);
    poner(16, ancho);
    poner(20, alto);
    poner(24, 0);
    poner(28, 0);
    poner(32, centi_mm_ancho);
    poner(36, centi_mm_alto);
    poner(44, 0x0001_0000);
    poner(48, total);
    poner(52, 3);
    poner(56, 0);
    poner(60, 0);
    poner(64, 0);
    poner(68, 0);
    poner(72, ancho);
    poner(76, alto);
    poner(80, mm_ancho);
    poner(84, mm_alto);
    poner(88, 0);
    poner(92, 0);
    poner(96, 0);
    poner(100, mm_ancho * 1000);
    poner(104, mm_alto * 1000);
    drop(poner);
    salida
}

/// Variante "doble": repite el encabezado, como el metarchivo que nanoCAD
/// escribe y que AutoCAD sí dibuja (dos EMR_HEADER seguidos).
pub fn emf_doble(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let base = emf_from_rgba(width, height, rgba);
    if base.len() < 8 {
        return base;
    }
    let tam_cabecera = u32::from_le_bytes([base[4], base[5], base[6], base[7]]) as usize;
    if tam_cabecera == 0 || tam_cabecera + 8 > base.len() {
        return base;
    }
    let mut salida = Vec::with_capacity(base.len() + tam_cabecera);
    salida.extend_from_slice(&base[..tam_cabecera]); // encabezado #1
    salida.extend_from_slice(&base[..tam_cabecera]); // encabezado #2 (duplicado)
    salida.extend_from_slice(&base[tam_cabecera..]); // registros + EOF
    let total = salida.len() as u32;
    let registros = 4u32; // los dos encabezados cuentan como registro
    for inicio in [0usize, tam_cabecera] {
        salida[inicio + 48..inicio + 52].copy_from_slice(&total.to_le_bytes());
        salida[inicio + 52..inicio + 56].copy_from_slice(&registros.to_le_bytes());
    }
    salida
}
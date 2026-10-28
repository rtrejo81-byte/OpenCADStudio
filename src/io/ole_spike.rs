//! Spike de OLE nativo: Windows crea el objeto incrustado del archivo por COM y
//! lo dibuja sobre un metarchivo. Ese EMF es lo que AutoCAD enseña dentro de un
//! OLE2FRAME, así que permite incrustar contenido de verdad.

/// Aspecto que se dibuja: el contenido del objeto.
const DVASPECT_CONTENT: u32 = 1;

/// Crea el objeto OLE del archivo y devuelve su representación como EMF.
#[cfg(target_os = "windows")]
pub fn emf_from_ole_file(ruta: &str, ancho_px: i32, alto_px: i32) -> Result<Vec<u8>, String> {
    use windows::core::{HSTRING, IUnknown, Interface, PCWSTR};
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        CloseEnhMetaFile, CreateEnhMetaFileW, DeleteEnhMetaFile, GetEnhMetaFileBits,
    };
    use windows::Win32::System::Ole::{
        IOleObject, OleCreateFromFile, OleDraw, OleInitialize, OleUninitialize, OLERENDER_NONE,
    };

    // Se ejecuta en un hilo propio: el hilo principal de la interfaz ya puede
    // tener COM inicializado en otro apartamento y OleInitialize fallaría.
    let ruta = ruta.to_string();
    std::thread::spawn(move || unsafe {
        let inicializado = match OleInitialize(None) {
            Ok(()) => true,
            Err(error) => {
                return Err(format!("OleInitialize: {error}"));
            }
        };
        let resultado = (|| -> Result<Vec<u8>, String> {
            let nombre = HSTRING::from(ruta);
            let mut crudo: *mut core::ffi::c_void = std::ptr::null_mut();
            OleCreateFromFile(
                // NULL: Windows deduce la clase del objeto por la extensión.
                std::ptr::null(),
                PCWSTR(nombre.as_ptr()),
                &IOleObject::IID,
                OLERENDER_NONE,
                std::ptr::null(),
                None,
                None,
                &mut crudo,
            )
            .map_err(|e| format!("OleCreateFromFile: {e}"))?;
            if crudo.is_null() {
                return Err("OleCreateFromFile devolvió un objeto nulo".to_string());
            }
            // OleCreateFromFile entrega un IOleObject; para dibujar se usa como
            // IUnknown (OleDraw pide esa interfaz base).
            let ole_object: IOleObject = IOleObject::from_raw(crudo);
            let objeto: IUnknown = ole_object.cast().map_err(|e| format!("cast: {e}"))?;

            let marco = RECT {
                left: 0,
                top: 0,
                right: ancho_px.max(64),
                bottom: alto_px.max(64),
            };
            let descripcion: Vec<u16> = "lgbcadstudio\0".encode_utf16().collect();
            let hdc = CreateEnhMetaFileW(
                None,
                PCWSTR::null(),
                Some(&marco),
                PCWSTR(descripcion.as_ptr()),
            );
            if hdc.is_invalid() {
                return Err("no se pudo crear el metarchivo".to_string());
            }
            OleDraw(&objeto, DVASPECT_CONTENT, hdc, &marco)
                .map_err(|e| format!("OleDraw: {e}"))?;
            let metarchivo = CloseEnhMetaFile(hdc);
            if metarchivo.is_invalid() {
                return Err("CloseEnhMetaFile falló".to_string());
            }
            let tamano = GetEnhMetaFileBits(metarchivo, None);
            if tamano == 0 {
                let _ = DeleteEnhMetaFile(Some(metarchivo));
                return Err("el metarchivo salió vacío".to_string());
            }
            let mut bytes = vec![0u8; tamano as usize];
            GetEnhMetaFileBits(metarchivo, Some(&mut bytes));
            let _ = DeleteEnhMetaFile(Some(metarchivo));
            Ok(bytes)
        })();
        if inicializado {
            OleUninitialize();
        }
        resultado
    })
    .join()
    .unwrap_or_else(|_| Err("el hilo COM terminó de forma inesperada".to_string()))
}

#[cfg(not(target_os = "windows"))]
pub fn emf_from_ole_file(_ruta: &str, _ancho: i32, _alto: i32) -> Result<Vec<u8>, String> {
    Err("OLE nativo solo está disponible en Windows.".to_string())
}
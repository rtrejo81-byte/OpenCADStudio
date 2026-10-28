# Objeto OLE que AutoCAD y nanoCAD sí dibujan

Referencia: `PLANOS PRUEBAS\prueba imagen nanocad.dwg` (abre bien en ambos).
Volcado: `dsh-work\_nano_ref.ole.txt`.

## estructura del objeto

En el registro DWG (OLE2FRAME) los campos van en este orden, que es el que
el codec ya usa:

    type(BS) mode(BS) data_size(BL) data(TF) lock_aspect(RC)

El bloque `data` son 128 bytes de envoltorio + un archivo compuesto OLE 2.0:

    0x5580                                  marca del envoltorio
    4 puntos 3D (f64)                        es DXF 10/11 (esquinas)
    registro de extensión (30 bytes)         presentación: PDF guarda 241,102,175,79...
    D0CF11E0 A1B11AE1                        archivo compuesto en el byte 128

## archivo compuesto (lo que falta en el nuestro)

CLSID raíz:

    {00000319-0000-0000-C000-000000000046}    "Picture (Enhanced Metafile)"
    bytes: 25,3,0,0, 0,0, 0,0, 192,0,0,0, 0,0,0,70

Flujos:

    \x01Ole       20 bytes: 1,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
    \x01CompObj   93 bytes (ver plantilla abajo)
    CONTENTS      bytes de la imagen: nanoCAD guarda un EMF (" EMF" en el byte 40)

Plantilla de `\x01CompObj`:

    1,0,254,255,3,10,0,0, 255,255,255,255,
    25,3,0,0,0,0,0,0,192,0,0,0,0,0,0,70,
    28,0,0,0,
    "Picture (Enhanced Metafile)" + 0x00,
    255,255,255,255,
    14,0,0,0, 0,0,0,0, 244,57,178,113, 0,0,0,0, 0,0,0,0, 0,0,0,0

## lo que escribe hoy la app (y por eso no se ve)

    CLSID raíz: ceros
    flujos:     sólo \x01Ole10Native

Sin CLSID ni `\x01CompObj` los CAD leen el marco y omiten la imagen.

## trabajo pendiente para el pegado incrustado

1. Generar un EMF con la imagen pegada (un EMR_STRETCHDIBITS basta).
2. Armar el compuesto con CLSID + `\x01Ole` + `\x01CompObj` + `CONTENTS`(EMF).
3. Mantener el envoltorio 0x5580 + los 4 puntos + el registro de 30 bytes.

El codec ya sabe escribir archivos compuestos con flujos arbitrarios, así que
el trabajo es armar los bytes, no cambiar el formato del DWG.
---

# Spike COM (creación real del objeto OLE) — estado 2026-10-28

## Resumen

El contenedor, la persistencia DWG y el empaquetado están resueltos y probados.
Lo único que falta es **crear el objeto** con Windows (COM) para obtener un
metarchivo **vectorial**, que es lo único que AutoCAD dibuja.

## Lo que ya funciona

| Pieza | Prueba |
|---|---|
| `OLE2FRAME` se escribe y se vuelve a leer | ida y vuelta con el codec vendorizado |
| AutoCAD dibuja NUESTRO contenedor | cruce B: contenedor nuestro + EMF de nanoCAD |
| Estructura del objeto | CLSID `{00000319-0000-0000-C000-000000000046}`, flujos `\x01Ole`, `\x01CompObj`, `CONTENTS` |
| Envoltorio DWG | marca `0x5580` + 4 puntos + registro de 30 bytes |
| Orden de campos en el registro | type, mode, data_size, data, lock_aspect (el original: mi "corrección" era errónea) |

## Lo que NO funciona

Todos los EMF **generados por nosotros** (GDI, .NET, plantilla de nanoCAD,
encabezado duplicado, 24/32 bpp, con y sin modo de mapa) son válidos y se
dibujan con .NET, pero **AutoCAD no los pinta**. Los EMF reales de nanoCAD sí.

Conclusión: AutoCAD necesita **contenido vectorial producido por un servidor OLE**,
no un mapa de bits empaquetado.

## Dato clave del portapapeles

Excel entrega solo `Bitmap` y `PNG` (verificado con `Clipboard.GetFormats()`):
**no hay `CF_ENHMETAFILE`**. El vectorial solo aparece cuando Windows activa
el servidor OLE del programa. De ahí la necesidad de COM.

## Spike implementado

Comando oculto (no afecta el uso normal):

    lgbcadstudio.exe --ole-spike <archivo-fuente> <salida.dwg>

Módulo: `src/io/ole_spike.rs`. Hace `OleInitialize` → `OleCreateFromFile` →
`OleDraw(DVASPECT_CONTENT)` sobre un DC de metarchivo → `GetEnhMetaFileBits` y
empaqueta con `build_embedded_emf`.

### Error actual

    OleCreateFromFile: El parámetro no es correcto. (0x80070057)

Se corrigieron ya: `riid` = `IID_IOleObject` (no IUnknown), `rclsid` = NULL,
`OLERENDER_NONE`, y la llamada corre en un **hilo propio STA**.

## Siguientes pasos (en este orden)

1. **Aislar el API del archivo.** Correr el spike con un tipo simple registrado
   (un `.bmp`, un `.txt`) para saber si falla el API o la clase del `.xlsx`.
2. **Ver qué clase detecta Windows:** registrar/loguear `GetClassFile` sobre el
   archivo; si devuelve CLSID_NULL, `OleCreateFromFile` devuelve E_INVALIDARG.
   En ese caso, pasar el CLSID explícito de Excel (`{00020820-0000-0000-C000-000000000046}`
   para hoja de cálculo) o usar `OleCreateFromFileEx`.
3. **Probar `OLERENDER_DRAW` con `FORMATETC` explícito** (CF_METAFILEPICT,
   DVASPECT_CONTENT, lindex -1, tymed GDI). La documentación lo exige para ese
   modo; con `OLERENDER_NONE` no se cachea presentación.
4. **Si COM sigue fallando para `.xlsx`:** alternativa válida es
   `OleCreateFromData` con el portapapeles (solo funciona si la app origen
   publica el objeto OLE, no el caso de Excel) o **objetos vinculados**
   (`OleCreateLinkToFile`), que son más simples y también dan caché vectorial.

## Cómo compilar y probar

    cargo build --release --offline          # el crate `windows` ya está en caché
    lgbcadstudio.exe --ole-spike "archivo.xlsx" "prueba.dwg"
    lgbcadstudio.exe --ole-extract "prueba.dwg" "contenido.bin"   # EMF del objeto
    lgbcadstudio.exe --ole-dump "prueba.dwg"                      # estructura OLE

Notas de la API (windows crate 0.62, todo offline):

- Feature necesaria: `Win32_System_Com_StructuredStorage` además de
  `Win32_System_Com`, `Win32_System_Ole`, `Win32_Graphics_Gdi`, `Win32_Foundation`.
- `IUnknown::IID` / `from_raw` vienen del trait `windows::core::Interface`.
- `HDC` y `HENHMETAFILE` son structs con `.is_invalid()`.
- `GetEnhMetaFileBits(hemf, Option<&mut [u8]>)`: dos argumentos, no tres.

## Camino práctico mientras tanto

- Imagen enlazada + `REUNIRIMAGENES` + ZIP de la carpeta.
- **Exportar PDF** para enviar: incluye todo y no depende de nada externo.

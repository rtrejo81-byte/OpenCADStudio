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
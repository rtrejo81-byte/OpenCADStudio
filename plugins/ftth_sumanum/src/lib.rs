use ocs_plugin_api::prelude::*;

#[no_mangle]
pub extern "C" fn ocs_plugin_init() -> *mut dyn Plugin {
    Box::into_raw(Box::new(FtthSumaNumPlugin::new()))
}

struct FtthSumaNumPlugin;

impl FtthSumaNumPlugin {
    fn new() -> Self { Self }
}

impl Plugin for FtthSumaNumPlugin {
    fn name(&self) -> &str { "FTTH_SUMANUM" }
    fn version(&self) -> &str { "1.0.0" }

    fn commands(&self) -> Vec<CommandDef> {
        vec![CommandDef {
            name: "FTTH_SUMANUM".to_string(),
            description: "Suma números dentro de textos (selección por capa o polígono)".to_string(),
            handler: Box::new(FtthSumaNumCommand),
        }]
    }
}

struct FtthSumaNumCommand;

impl CommandHandler for FtthSumaNumCommand {
    fn execute(&self, ctx: &mut CommandContext) -> CommandResult {
        // 1. Pedir al usuario que seleccione los textos
        ctx.prompt("Seleccione los textos a sumar:");
        let selection = ctx.select_entities(&["TEXT", "MTEXT"])?;

        if selection.is_empty() {
            ctx.print("No se seleccionaron textos.");
            return Ok(());
        }

        let mut suma_total = 0.0;
        let mut contador = 0;
        let mut textos_sumados = Vec::new();

        ctx.print("--- VALORES ENCONTRADOS ---");

        // 2. Recorrer los textos y extraer los números
        for handle in selection {
            if let Some(entity) = ctx.get_entity(handle) {
                if let EntityData::Text(text_data) = entity.data {
                    let contenido = text_data.text;
                    
                    // Lógica de extracción de números (similar a tu LISP)
                    if let Some(numero) = extraer_numero(&contenido) {
                        suma_total += numero;
                        contador += 1;
                        textos_sumados.push(handle);
                        ctx.print(&format!("  {} = {}", contenido, numero));
                    } else {
                        ctx.print(&format!("  {} = (sin número)", contenido));
                    }
                }
            }
        }

        // 3. Mostrar resultados
        ctx.print("---------------------------");
        ctx.print(&format!("TEXTOS PROCESADOS: {}", contador));
        ctx.print(&format!("SUMA TOTAL: {:.1}", suma_total));

        // 4. Preguntar si quiere resaltar los textos
        if ctx.confirm("¿Desea resaltar los textos que se sumaron?")? {
            ctx.highlight_entities(&textos_sumados)?;
            ctx.print("Presione ENTER para limpiar el resaltado...");
            ctx.wait_for_enter()?;
            ctx.clear_highlight()?;
        }

        ctx.print("--- POWERED BY ROBERTO TREJO ---");
        Ok(())
    }
}

// Función auxiliar para extraer el primer número de un texto (traducción de tu LISP)
fn extraer_numero(texto: &str) -> Option<f64> {
    let mut num_str = String::new();
    let mut punto_encontrado = false;
    let mut encontrado = false;

    for c in texto.chars() {
        if c.is_ascii_digit() {
            num_str.push(c);
            encontrado = true;
        } else if c == '.' && !punto_encontrado {
            num_str.push(c);
            punto_encontrado = true;
            encontrado = true;
        } else if encontrado {
            break; // Si ya encontramos un número, paramos en el primer carácter no numérico
        }
    }

    num_str.parse::<f64>().ok()
}
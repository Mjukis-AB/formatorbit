/**
 * Formatorbit C API
 *
 * A cross-platform data format converter library.
 * Automatically detects and converts data between formats.
 *
 * All functions returning strings allocate memory that must be freed
 * with formatorbit_free_string(). Null pointers are handled gracefully.
 *
 * Example usage:
 *
 *   char *result = formatorbit_convert_all("691E01B8");
 *   // Parse JSON result...
 *   formatorbit_free_string(result);
 */

#ifndef FORMATORBIT_H
#define FORMATORBIT_H

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * Version & Info
 * ============================================================================ */

/**
 * Get the library version string (e.g., "0.3.0").
 *
 * @return Newly allocated string. Caller must free with formatorbit_free_string().
 */
char *formatorbit_version(void);

/**
 * Get information about all supported formats as JSON.
 *
 * Returns a JSON array of format info objects:
 * [
 *   {
 *     "id": "hex",
 *     "name": "Hexadecimal",
 *     "category": "Encoding",
 *     "description": "Hexadecimal byte encoding",
 *     "examples": ["691E01B8", "0x691E01B8"],
 *     "aliases": ["h", "x"]
 *   },
 *   ...
 * ]
 *
 * @return Newly allocated JSON string. Caller must free with formatorbit_free_string().
 */
char *formatorbit_list_formats(void);

/* ============================================================================
 * Conversion Functions
 * ============================================================================ */

/**
 * Convert input and return JSON with all results.
 *
 * Returns all possible interpretations and their conversions, sorted by
 * confidence (highest first). Each interpretation includes all possible
 * conversions sorted by priority (structured data first).
 *
 * Example output:
 * [
 *   {
 *     "input": "691E01B8",
 *     "interpretation": {
 *       "value": {"type": "Bytes", "value": [105, 30, 1, 184]},
 *       "source_format": "hex",
 *       "confidence": 0.92,
 *       "description": "4 bytes"
 *     },
 *     "conversions": [
 *       {
 *         "value": {"type": "String", "value": "105.30.1.184"},
 *         "target_format": "ipv4",
 *         "display": "105.30.1.184",
 *         "path": ["ipv4"],
 *         "is_lossy": false,
 *         "priority": "Semantic"
 *       },
 *       ...
 *     ]
 *   }
 * ]
 *
 * @param input  Null-terminated input string to convert, or NULL.
 * @return Newly allocated JSON string. Caller must free with formatorbit_free_string().
 */
char *formatorbit_convert_all(const char *input);

/**
 * Convert input using only specific formats.
 *
 * @param input   Null-terminated input string to convert, or NULL.
 * @param formats Comma-separated list of format IDs or aliases (e.g., "hex,uuid,ts"),
 *                or NULL/empty for all formats.
 * @return Newly allocated JSON string. Caller must free with formatorbit_free_string().
 */
char *formatorbit_convert_filtered(const char *input, const char *formats);

/**
 * Convert input and return only the highest-confidence result.
 *
 * More efficient when you only need the best interpretation.
 *
 * @param input  Null-terminated input string to convert, or NULL.
 * @return Newly allocated JSON string with single result object, or "null" if
 *         no interpretation found. Caller must free with formatorbit_free_string().
 */
char *formatorbit_convert_first(const char *input);

/**
 * Convert input, forcing interpretation as a specific format.
 *
 * Skips auto-detection and treats the input as the specified format.
 * Useful when you know the input format and want to see conversions.
 *
 * @param input       Null-terminated input string to convert, or NULL.
 * @param from_format Format ID to force (e.g., "hex"), or NULL/empty for auto-detect.
 * @return Newly allocated JSON string. Caller must free with formatorbit_free_string().
 */
char *formatorbit_convert_from(const char *input, const char *from_format);

/* ============================================================================
 * Memory Management
 * ============================================================================ */

/**
 * Free a string allocated by formatorbit functions.
 *
 * @param s  Pointer returned by a formatorbit function, or NULL (safe to call).
 *           After calling, the pointer is invalid and must not be used.
 */
void formatorbit_free_string(char *s);

#ifdef __cplusplus
}
#endif

#endif /* FORMATORBIT_H */

package com.camarelol.catalog;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Minimal JSON reader/writer with no external dependencies.
 *
 * Supports exactly what the catalog protocol needs: objects, arrays, strings,
 * numbers, booleans and null. Enough for NDJSON, nothing more.
 */
public final class Json {

    public static final class JsonException extends RuntimeException {
        public JsonException(String message) {
            super(message);
        }
    }

    // ------------------------------------------------------------------
    // reading
    // ------------------------------------------------------------------

    public static Object parse(String text) {
        Parser parser = new Parser(text);
        Object value = parser.parseValue();
        parser.skipWhitespace();
        if (!parser.atEnd()) {
            throw new JsonException("trailing data after JSON value");
        }
        return value;
    }

    public static Map<String, Object> parseObject(String text) {
        Object value = parse(text);
        if (!(value instanceof Map)) {
            throw new JsonException("expected a JSON object");
        }
        @SuppressWarnings("unchecked")
        Map<String, Object> map = (Map<String, Object>) value;
        return map;
    }

    public static String asString(Object value) {
        if (value == null) {
            return null;
        }
        return value.toString();
    }

    public static boolean asBool(Object value) {
        return Boolean.TRUE.equals(value) || "true".equals(value);
    }

    public static int asInt(Object value) {
        if (value instanceof Number n) {
            return n.intValue();
        }
        if (value instanceof String s) {
            return Integer.parseInt(s);
        }
        throw new JsonException("expected an integer");
    }

    public static List<Object> asArray(Object value) {
        if (!(value instanceof List)) {
            throw new JsonException("expected an array");
        }
        return (List<Object>) value;
    }

    private static final class Parser {
        private final String text;
        private int pos;

        Parser(String text) {
            this.text = text;
        }

        boolean atEnd() {
            return pos >= text.length();
        }

        void skipWhitespace() {
            while (pos < text.length() && Character.isWhitespace(text.charAt(pos))) {
                pos++;
            }
        }

        Object parseValue() {
            skipWhitespace();
            if (atEnd()) {
                throw new JsonException("unexpected end of input");
            }
            char c = text.charAt(pos);
            switch (c) {
                case '{':
                    return parseObject();
                case '[':
                    return parseArray();
                case '"':
                    return parseString();
                case 't':
                    expect("true");
                    return Boolean.TRUE;
                case 'f':
                    expect("false");
                    return Boolean.FALSE;
                case 'n':
                    expect("null");
                    return null;
                default:
                    return parseNumber();
            }
        }

        private void expect(String word) {
            if (!text.startsWith(word, pos)) {
                throw new JsonException("expected '" + word + "'");
            }
            pos += word.length();
        }

        private Map<String, Object> parseObject() {
            expect("{");
            Map<String, Object> map = new LinkedHashMap<>();
            skipWhitespace();
            if (!atEnd() && text.charAt(pos) == '}') {
                pos++;
                return map;
            }
            while (true) {
                skipWhitespace();
                if (atEnd() || text.charAt(pos) != '"') {
                    throw new JsonException("expected a string key");
                }
                String key = parseString();
                skipWhitespace();
                if (atEnd() || text.charAt(pos) != ':') {
                    throw new JsonException("expected ':'");
                }
                pos++;
                Object value = parseValue();
                map.put(key, value);
                skipWhitespace();
                if (atEnd()) {
                    throw new JsonException("unterminated object");
                }
                char c = text.charAt(pos);
                if (c == ',') {
                    pos++;
                    continue;
                }
                if (c == '}') {
                    pos++;
                    return map;
                }
                throw new JsonException("expected ',' or '}'");
            }
        }

        private List<Object> parseArray() {
            expect("[");
            List<Object> list = new ArrayList<>();
            skipWhitespace();
            if (!atEnd() && text.charAt(pos) == ']') {
                pos++;
                return list;
            }
            while (true) {
                list.add(parseValue());
                skipWhitespace();
                if (atEnd()) {
                    throw new JsonException("unterminated array");
                }
                char c = text.charAt(pos);
                if (c == ',') {
                    pos++;
                    continue;
                }
                if (c == ']') {
                    pos++;
                    return list;
                }
                throw new JsonException("expected ',' or ']'");
            }
        }

        private String parseString() {
            expect("\"");
            StringBuilder sb = new StringBuilder();
            while (true) {
                if (atEnd()) {
                    throw new JsonException("unterminated string");
                }
                char c = text.charAt(pos++);
                if (c == '"') {
                    return sb.toString();
                }
                if (c == '\\') {
                    if (atEnd()) {
                        throw new JsonException("unterminated escape");
                    }
                    char e = text.charAt(pos++);
                    switch (e) {
                        case '"' -> sb.append('"');
                        case '\\' -> sb.append('\\');
                        case '/' -> sb.append('/');
                        case 'n' -> sb.append('\n');
                        case 'r' -> sb.append('\r');
                        case 't' -> sb.append('\t');
                        case 'b' -> sb.append('\b');
                        case 'f' -> sb.append('\f');
                        case 'u' -> {
                            if (pos + 4 > text.length()) {
                                throw new JsonException("bad unicode escape");
                            }
                            sb.append((char) Integer.parseInt(text.substring(pos, pos + 4), 16));
                            pos += 4;
                        }
                        default -> throw new JsonException("bad escape: \\" + e);
                    }
                } else {
                    sb.append(c);
                }
            }
        }

        private Object parseNumber() {
            int start = pos;
            while (!atEnd()) {
                char c = text.charAt(pos);
                if (c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E'
                        || (c >= '0' && c <= '9')) {
                    pos++;
                } else {
                    break;
                }
            }
            if (start == pos) {
                throw new JsonException("expected a value");
            }
            String raw = text.substring(start, pos);
            try {
                return Integer.parseInt(raw);
            } catch (NumberFormatException ignored) {
                // fall through
            }
            try {
                return Long.parseLong(raw);
            } catch (NumberFormatException ignored) {
                // fall through
            }
            return Double.parseDouble(raw);
        }
    }

    // ------------------------------------------------------------------
    // writing
    // ------------------------------------------------------------------

    public static String write(Object value) {
        StringBuilder sb = new StringBuilder();
        writeValue(sb, value);
        return sb.toString();
    }

    private static void writeValue(StringBuilder sb, Object value) {
        if (value == null) {
            sb.append("null");
        } else if (value instanceof String s) {
            writeString(sb, s);
        } else if (value instanceof Boolean || value instanceof Number) {
            sb.append(value);
        } else if (value instanceof Map<?, ?> map) {
            sb.append('{');
            boolean first = true;
            for (Map.Entry<?, ?> entry : map.entrySet()) {
                if (!first) {
                    sb.append(',');
                }
                first = false;
                writeString(sb, String.valueOf(entry.getKey()));
                sb.append(':');
                writeValue(sb, entry.getValue());
            }
            sb.append('}');
        } else if (value instanceof List<?> list) {
            sb.append('[');
            boolean first = true;
            for (Object item : list) {
                if (!first) {
                    sb.append(',');
                }
                first = false;
                writeValue(sb, item);
            }
            sb.append(']');
        } else {
            writeString(sb, String.valueOf(value));
        }
    }

    private static void writeString(StringBuilder sb, String s) {
        sb.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"' -> sb.append("\\\"");
                case '\\' -> sb.append("\\\\");
                case '\n' -> sb.append("\\n");
                case '\r' -> sb.append("\\r");
                case '\t' -> sb.append("\\t");
                default -> sb.append(c);
            }
        }
        sb.append('"');
    }

    private Json() {
    }
}

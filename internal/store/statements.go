package store

import "strings"

// statements splits SQL at the semicolons outside quotes and comments, and
// drops the parts that hold only spaces and comments. The sqlite plugin
// runs one statement at a time.
func statements(sql string) []string {
	var out []string
	start, code := 0, false
	for i := 0; i < len(sql); i++ {
		switch c := sql[i]; {
		case c == '\'' || c == '"' || c == '`':
			i = skipPast(sql, i+1, string(c))
			code = true
		case c == '[':
			i = skipPast(sql, i+1, "]")
			code = true
		case strings.HasPrefix(sql[i:], "--"):
			i = skipPast(sql, i+2, "\n")
		case strings.HasPrefix(sql[i:], "/*"):
			i = skipPast(sql, i+2, "*/")
		case c == ';':
			if code {
				out = append(out, sql[start:i+1])
			}
			start, code = i+1, false
		case c != ' ' && c != '\t' && c != '\n' && c != '\r' && c != '\f':
			code = true
		}
	}
	if code {
		out = append(out, sql[start:])
	}
	return out
}

// skipPast returns the index of the last byte of the first end at or after
// from, or the last index when there is none. A doubled quote inside a
// quoted name or string is its escape and is skipped as two closings.
func skipPast(sql string, from int, end string) int {
	i := strings.Index(sql[from:], end)
	if i < 0 {
		return len(sql) - 1
	}
	return from + i + len(end) - 1
}

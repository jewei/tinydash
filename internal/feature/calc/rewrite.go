package calc

import (
	"fmt"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"
)

// The rewrites below turn a query into the input that the evaluator reads.
// They are the Rust app's, which wrote fend's syntax; the evaluator reads the
// same syntax, so the answers stay the same.

const (
	// maxInput is the longest query, in characters, that the calculator
	// reads.
	maxInput = 500
	// maxParentheses is the most parentheses in a query.
	maxParentheses = 32
	// maxDepth bounds the nesting of the input, after the rewrites that add
	// parentheses: each `)` and the deepest group count.
	maxDepth = 64
)

// input is the query in the evaluator's syntax, or false when it binds a
// name, when the evaluator would read a percentage in it differently from
// people, or when it is too long or too deep.
func input(query string) (string, bool) {
	ascii := asciiSigns(query)
	if utf8.RuneCountInString(query) > maxInput ||
		strings.Count(query, "(")+strings.Count(query, ")") > maxParentheses ||
		bindsName(ascii) {
		return "", false
	}
	in, ok := currencyShorthand(ascii)
	if !ok {
		if in, ok = percentage(wordOperators(euroAmounts(ascii))); !ok {
			return "", false
		}
	}
	return in, shallow(in)
}

// asciiSigns writes fend's other spellings of its signs in ASCII, so that
// the checks here see every sign. `×*` stays two products: the space keeps
// them apart.
func asciiSigns(query string) string {
	var b strings.Builder
	for _, r := range query {
		switch r {
		case '−':
			b.WriteByte('-')
		case '×', '✕':
			b.WriteString(" *")
		case '÷', '∕':
			b.WriteByte('/')
		case '≠':
			b.WriteString("!=")
		case 'λ':
			b.WriteByte('\\')
		default:
			b.WriteRune(r)
		}
	}
	return b.String()
}

// bindsName reports whether input binds a name, as fend can: a lambda in any
// of its spellings (`\x.x + 1`, `x: x + 1`, `x => x + 1`), or an assignment
// such as `a = 5`. A launcher needs neither, and a function that calls
// itself never ends.
func bindsName(input string) bool {
	for i := 0; i < len(input); i++ {
		next := func(c byte) bool { return i+1 < len(input) && input[i+1] == c }
		switch input[i] {
		case '\\', ':':
			return true
		case '!', '<', '>':
			// `!=` compares.
			if next('=') {
				i++
			}
		case '=':
			// `==` compares; a lone `=` assigns, and `=>` is a lambda.
			if !next('=') {
				return true
			}
			i++
		}
	}
	return false
}

// shallow reports whether input nests at most maxDepth deep.
func shallow(input string) bool {
	open, deepest, closed := 0, 0, 0
	for _, r := range input {
		switch r {
		case '(':
			open++
			deepest = max(deepest, open)
		case ')':
			closed++
			if open > 0 {
				open--
			}
		}
	}
	return closed+deepest <= maxDepth
}

// currencyShorthand reads `100 USD MYR` as `100 USD to MYR`.
func currencyShorthand(query string) (string, bool) {
	words := strings.Fields(query)
	if len(words) != 3 || !isFloat(words[0]) || !isCode(words[1]) || !isCode(words[2]) {
		return "", false
	}
	return words[0] + " " + words[1] + " to " + words[2], true
}

func isCode(code string) bool {
	return len(code) == 3 && strings.Trim(code, "ABCDEFGHIJKLMNOPQRSTUVWXYZ") == ""
}

// isFloat reports whether text is a float as Rust reads one: Go also reads
// hex floats and underscores, which Rust does not.
func isFloat(text string) bool {
	if strings.ContainsAny(text, "xX_") {
		return false
	}
	_, err := strconv.ParseFloat(text, 64)
	return err == nil
}

// euroAmounts reads `€20` as `20 EUR`; `$20` and `£20` read as they are.
func euroAmounts(query string) string {
	if !strings.Contains(query, "€") {
		return query
	}
	var b strings.Builder
	rest := query
	for {
		at := strings.Index(rest, "€")
		if at < 0 {
			break
		}
		b.WriteString(rest[:at])
		after := rest[at+len("€"):]
		amount := strings.IndexFunc(after, func(r rune) bool {
			return !(r < utf8.RuneSelf && (unicode.IsLetter(r) || unicode.IsDigit(r)) || r == '.' || r == ',')
		})
		if amount < 0 {
			amount = len(after)
		}
		if after != "" && isDigit(rune(after[0])) {
			b.WriteString(after[:amount] + " EUR")
			rest = after[amount:]
		} else {
			b.WriteString("€")
			rest = after
		}
	}
	b.WriteString(rest)
	return b.String()
}

// wordOperators reads `plus`, `minus`, `times`, `multiplied by`, and
// `divided by` as signs, and `percent` as `%`.
func wordOperators(query string) string {
	words := strings.Fields(query)
	is := func(i int, word string) bool { return i < len(words) && asciiEqualFold(words[i], word) }
	var out []string
	changed := false
	for i := 0; i < len(words); {
		sign, used := words[i], 0
		switch {
		case is(i, "plus"):
			sign, used = "+", 1
		case is(i, "minus"):
			sign, used = "-", 1
		case is(i, "times"):
			sign, used = "*", 1
		case is(i, "multiplied") && is(i+1, "by"):
			sign, used = "*", 2
		case is(i, "divided") && is(i+1, "by"):
			sign, used = "/", 2
		case is(i, "percent"):
			sign, used = "%", 1
		default:
			// `20percent`
			if number, ok := stripSuffixFold(words[i], "percent"); ok && isNumber(number) {
				out = append(out, number)
				sign, used = "%", 1
			}
		}
		changed = changed || used > 0
		out = append(out, sign)
		i += max(used, 1)
	}
	if !changed {
		return query
	}
	return strings.Join(out, " ")
}

// percentage writes percentages as people say them in the evaluator's
// syntax, or returns false for a percentage in any other shape. The
// evaluator, like fend, reads `30 + 20%` as 30.2 and `200 / 10%` as 20%. A
// wrong answer is worse than none.
func percentage(query string) (string, bool) {
	if in, ok := percentageQuestion(query); ok {
		return in, true
	}
	if in, ok := percentageSteps(query); ok {
		return in, true
	}
	in := percentagePhrases(query)
	return in, !mixesPercentage(in)
}

// factors are `10% of`, `10% on`, and `10% off` as a factor: `10 / 100`,
// `1 + 10 / 100`, and `1 - 10 / 100`.
var factors = []struct{ word, factor string }{{"of", ""}, {"on", "1 + "}, {"off", "1 - "}}

// percentageQuestion reads a question about a percentage, as a whole:
// `20 is what % of 200`, `50 to 75 is what %`, `3/20 is what %`, or
// `20 is 10% of what`. Each part is a plain amount, or a ratio of two in
// `3/20 is what %`. `to` and `in` read as `as`, so `0.35 to %` is `0.35 as %`.
func percentageQuestion(query string) (string, bool) {
	query = strings.TrimSpace(query)
	for _, ending := range []string{" is what %", " as %", " as a %", " to %", " in %"} {
		left, ok := stripSuffixFold(query, ending)
		if !ok {
			continue
		}
		if from, to, ok := around(left, " to "); ok {
			return fmt.Sprintf("((%s) - (%s)) / (%s) to %%", to, from, from), isAmount(from) && isAmount(to)
		}
		amounts := isAmount(left)
		if a, b, ok := strings.Cut(left, "/"); ok {
			amounts = isAmount(a) && isAmount(b)
		}
		return "(" + left + ") to %", amounts
	}
	ratios := []struct {
		separator, format string
	}{
		{" is what % of ", "(%s) / (%s) to %%"},
		{" as a % of ", "(%s) / (%s) to %%"},
		{" as % of ", "(%s) / (%s) to %%"},
		{" is what % off ", "(1 - (%s) / (%s)) to %%"},
		{" is what % on ", "((%s) / (%s) - 1) to %%"},
	}
	for _, ratio := range ratios {
		if a, b, ok := around(query, ratio.separator); ok {
			return fmt.Sprintf(ratio.format, a, b), isAmount(a) && isAmount(b)
		}
	}
	// `20 is 10% of what`: the number that 20 is 10% of.
	left, ok := stripSuffixFold(query, " what")
	if !ok {
		return "", false
	}
	for _, f := range factors {
		phrase, ok := stripSuffixFold(left, f.word)
		if !ok {
			continue
		}
		phrase, ok = strings.CutSuffix(phrase, "%")
		if !ok {
			continue
		}
		value, percent, ok := around(phrase, " is ")
		if ok && isAmount(value) && isNumber(percent) {
			return fmt.Sprintf("(%s) / (%s%s / 100)", value, f.factor, percent), true
		}
	}
	return "", false
}

// percentageSteps reads `$30 + 20%`, which adds 20% of $30, and
// `200 - 10% - 5%`, which takes 5% off 180: each percentage is of the value
// before it. The whole query is a plain amount and such steps.
func percentageSteps(query string) (string, bool) {
	query = strings.TrimSpace(query)
	n, ok := amountLen(query)
	if !ok {
		return "", false
	}
	amount := query[:n]
	rest := trimStart(query[n:])
	if rest == "" {
		return "", false
	}
	// Each step multiplies the one before it, so a chain adds no nesting.
	in := "(" + amount + ")"
	for rest != "" {
		sign := rest[:1]
		if sign != "+" && sign != "-" {
			return "", false
		}
		step := trimStart(rest[1:])
		n, ok := numberLen(step)
		if !ok {
			return "", false
		}
		percent := step[:n]
		if rest, ok = strings.CutPrefix(trimStart(step[n:]), "%"); !ok {
			return "", false
		}
		rest = trimStart(rest)
		in += " * (1 " + sign + " " + percent + " / 100)"
	}
	return in, true
}

// percentagePhrases writes `10% of 200`, `10% on 200`, and `10% off 200` as
// `((10 / 100) * (200))`, `((1 + 10 / 100) * (200))`, and
// `((1 - 10 / 100) * (200))`: one value, where it stands, so
// `10% of 200 + 50` is 70. Only where that is the one reading (see
// startsValue and endsValue): after `/` or a power, fend takes a percentage
// of all that comes before it, so `1/2% of 200` is 1, and after a sign
// alone, `-10% off 200` could be -180 or 220.
func percentagePhrases(query string) string {
	var b strings.Builder
	// b holds query up to copied, rewritten.
	copied := 0
	for at := 0; at < len(query); at++ {
		if query[at] != '%' || copied > at+1 {
			// A `%` before copied is in a phrase already rewritten.
			continue
		}
		before, ok := beforePercentage(query[copied : at+1])
		if !ok {
			continue
		}
		percent := strings.TrimSpace(query[copied+len(before) : at])
		after := trimStart(query[at+1:])
		letters := strings.IndexFunc(after, func(r rune) bool { return !isAlphabetic(r) })
		if letters < 0 {
			letters = len(after)
		}
		word, rest := after[:letters], after[letters:]
		factor, found := "", false
		for _, f := range factors {
			if asciiEqualFold(word, f.word) {
				factor, found = f.factor, true
				break
			}
		}
		if !found {
			continue
		}
		amount := trimStart(rest)
		n, ok := amountLen(amount)
		if !ok {
			continue
		}
		// What the user typed before the percentage, so that an amount in
		// a phrase already rewritten is a value too.
		if isNumber(percent) && len(amount) < len(rest) &&
			startsValue(query[:copied+len(before)]) && endsValue(amount[n:]) {
			b.WriteString(before)
			fmt.Fprintf(&b, "((%s%s / 100) * (%s))", factor, percent, amount[:n])
			copied = len(query) - len(amount) + n
		}
	}
	if copied == 0 {
		return query
	}
	return b.String() + query[copied:]
}

// startsValue reports whether a value after text stands on its own: at the
// start, after `(`, or after a `+`, `-`, or `*` that follows a plain amount
// or a `)`. text is what the user typed, so a phrase already rewritten ends
// with its amount. After anything else, the sign can belong to the
// percentage, as after the operator word `mod` in `400 mod +10% off 200`.
func startsValue(text string) bool {
	text = trimEnd(text)
	if text != "" && strings.ContainsRune("+-*", rune(text[len(text)-1])) {
		left := trimEnd(text[:len(text)-1])
		return strings.HasSuffix(left, ")") || endsWithAmount(left)
	}
	return text == "" || strings.HasSuffix(text, "(")
}

// endsWithAmount reports whether text ends with a plain amount (see
// amountLen) that reads as one: it starts at the start of text, after a
// space, or after one of `()+-*/^`. So `1e1`, `0x10`, and `5²` do not end
// with one.
func endsWithAmount(text string) bool {
	start := func(text string) int {
		return len(strings.TrimRightFunc(text, func(r rune) bool {
			return !unicode.IsSpace(r) && !strings.ContainsRune("()+-*/^", r)
		}))
	}
	at := start(text)
	// A unit after a space, such as `kg` in `5 kg`.
	if unit := text[at:]; unit != "" && strings.IndexFunc(unit, func(r rune) bool { return !isAlphabetic(r) }) < 0 {
		at = start(trimEnd(text[:at]))
	}
	return isAmount(text[at:])
}

// The words that fend-core reads as operators, in any case.
var operatorWordList = []string{"to", "as", "in", "per", "of", "mod", "and", "or", "xor", "choose", "nCr", "permute", "nPr"}

func isOperatorWord(word string) bool {
	for _, operator := range operatorWordList {
		if asciiEqualFold(word, operator) {
			return true
		}
	}
	return false
}

// endsValue reports whether a value before text stands on its own: at the
// end, or before `)`, `+`, `-`, `/`, or a `*` that is not a power.
func endsValue(text string) bool {
	text = trimStart(text)
	return text == "" || strings.ContainsRune(")+-/", rune(text[0])) ||
		text[0] == '*' && !strings.HasPrefix(text, "**")
}

// mixesPercentage reports whether input has a percentage with something
// else, which fend and the evaluator read differently from people:
// `100 + 10% + 5` as 105.1, `200 / 10%` as 20%. A percentage alone, such as
// `2e-3%`, and a remainder, such as `10 % 3`, are fine.
func mixesPercentage(input string) bool {
	// `20 percent` is `20 %` by now, but `(20 percent)` is not.
	if strings.Contains(asciiLower(input), "percent") {
		return true
	}
	before, ok := beforePercentage(strings.TrimSpace(input))
	if ok {
		switch strings.TrimSpace(before) {
		case "", "+", "-":
			return false
		}
	}
	// A `%` before a number or `(` is a remainder.
	for at := 0; at < len(input); at++ {
		if input[at] != '%' {
			continue
		}
		after := trimStart(input[at+1:])
		if after == "" || !(isDigit(rune(after[0])) || after[0] == '.' || after[0] == '(') {
			return true
		}
	}
	return false
}

// beforePercentage returns the text before the percentage, such as `10%`
// or `1e-2 %`, that ends text.
func beforePercentage(text string) (string, bool) {
	number, ok := strings.CutSuffix(text, "%")
	if !ok {
		return "", false
	}
	number = trimEnd(number)
	digit := func(r rune) bool { return isDigit(r) || r == '.' || r == ',' }
	rest := strings.TrimRightFunc(number, digit)
	// In `1e-2`, the sign belongs to the exponent.
	unsigned, _ := cutSuffixAny(rest, "+-")
	if mantissa, ok := cutSuffixAny(unsigned, "eE"); ok && mantissa != "" && isDigit(rune(mantissa[len(mantissa)-1])) {
		rest = strings.TrimRightFunc(mantissa, digit)
	}
	if !strings.ContainsFunc(number[len(rest):], isDigit) {
		return "", false
	}
	return rest, true
}

func cutSuffixAny(text, chars string) (string, bool) {
	if text != "" && strings.ContainsRune(chars, rune(text[len(text)-1])) {
		return text[:len(text)-1], true
	}
	return text, false
}

// isAmount reports whether text is a plain amount: a number with an
// optional sign, currency sign, and unit, such as `200`, `-$5`, or `5 kg`.
func isAmount(text string) bool {
	text = strings.TrimSpace(text)
	n, ok := amountLen(text)
	return ok && n == len(text)
}

// amountLen is the length of the plain amount at the start of text.
func amountLen(text string) (int, bool) {
	unsigned := text
	if unsigned != "" && (unsigned[0] == '+' || unsigned[0] == '-') {
		unsigned = unsigned[1:]
	}
	number := unsigned
	for _, sign := range []string{"$", "£", "¥"} {
		if after, ok := strings.CutPrefix(number, sign); ok {
			number = after
			break
		}
	}
	n, ok := numberLen(number)
	if !ok {
		return 0, false
	}
	rest := number[n:]
	// A unit is a word after a space, such as `kg` or `EUR`: in `2e-3`, `e`
	// is part of the number, and in `5 per`, `per` is fend's `/`.
	unit := trimStart(rest)
	letters := strings.IndexFunc(unit, func(r rune) bool { return !isAlphabetic(r) })
	if letters < 0 {
		letters = len(unit)
	}
	if letters > 0 && len(unit) < len(rest) && endsToken(unit[letters:]) && !isOperatorWord(unit[:letters]) {
		rest = unit[letters:]
	}
	return len(text) - len(rest), true
}

// numberLen is the length of the plain number at the start of text, such
// as `20`, `12.5`, or `1,000`, when the number ends there too: not in `2e3`,
// `2(`, or `2^3`.
func numberLen(text string) (int, bool) {
	n := strings.IndexFunc(text, func(r rune) bool { return !(isDigit(r) || r == '.' || r == ',') })
	if n < 0 {
		n = len(text)
	}
	return n, isNumber(text[:n]) && !strings.HasPrefix(text, ",") && endsToken(text[n:])
}

// endsToken reports whether a number or a word ends before rest.
func endsToken(rest string) bool {
	r, _ := utf8.DecodeRuneInString(rest)
	return rest == "" || unicode.IsSpace(r) || strings.ContainsRune("+-*/)%", r)
}

// stripSuffixFold returns the text before suffix, matched in any ASCII
// case, when some is left.
func stripSuffixFold(text, suffix string) (string, bool) {
	at := len(text) - len(suffix)
	if at < 0 || at < len(text) && !utf8.RuneStart(text[at]) || !asciiEqualFold(text[at:], suffix) {
		return "", false
	}
	left := strings.TrimSpace(text[:at])
	return left, left != ""
}

// around returns the text before and after the first separator, matched
// in any ASCII case, when both are there.
func around(text, separator string) (string, string, bool) {
	// ASCII lowercase keeps every byte offset.
	at := strings.Index(asciiLower(text), separator)
	if at < 0 {
		return "", "", false
	}
	left, right := strings.TrimSpace(text[:at]), strings.TrimSpace(text[at+len(separator):])
	return left, right, left != "" && right != ""
}

// isNumber reports whether text is a plain number, such as `20`, `12.5`, or
// `1,000`.
func isNumber(text string) bool {
	return strings.ContainsFunc(text, isDigit) &&
		strings.Trim(text, "0123456789.,") == ""
}

func asciiLower(text string) string {
	b := []byte(text)
	for i, c := range b {
		if 'A' <= c && c <= 'Z' {
			b[i] = c + 'a' - 'A'
		}
	}
	return string(b)
}

func asciiEqualFold(a, b string) bool {
	return len(a) == len(b) && asciiLower(a) == asciiLower(b)
}

func trimStart(text string) string { return strings.TrimLeftFunc(text, unicode.IsSpace) }

func trimEnd(text string) string { return strings.TrimRightFunc(text, unicode.IsSpace) }

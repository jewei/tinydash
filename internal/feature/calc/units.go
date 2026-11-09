package calc

import (
	"math/big"
	"strings"
)

// The base quantities that the unit table measures.
const (
	dimLength = iota
	dimMass
	dimTime
	dimTemperature
	dimData
	dimMoney
	dimCount
)

type dims [dimCount]int

// unit is one named unit, with any prefix already applied.
type unit struct {
	singular, plural string
	dims             dims
	// scale is the size in base units: meter, kilogram, second, kelvin, bit,
	// or euro.
	scale *big.Rat
	// offset is where the scale's zero is, in kelvin, for celsius and
	// fahrenheit. Only conversions apply it, as fend does.
	offset *big.Rat
	// currency is the ISO code whose rate sets scale.
	currency string
	percent  bool
}

// prefixes says which prefixes a unit takes: short ones such as `k` in
// `km`, long ones such as `kilo` in `kilometer`.
type prefixes uint8

const (
	noPrefix prefixes = iota
	shortPrefix
	longPrefix
)

type unitDef struct {
	singular, plural string
	prefixes         prefixes
	dim              int
	// scale is a rational, such as "0.45359237" or "1/16"; "" is 1.
	scale  string
	offset string
	// perSecond makes a data unit a data rate.
	perSecond bool
}

// units are the units of length, mass, time, temperature, and data that
// people convert, with fend-core's names, plurals, and sizes. fend has many
// more; the port answers these. Order matters as in fend: an exact name wins,
// and a name that matches only in another case must match one unit.
var units = []unitDef{
	{"second", "seconds", longPrefix, dimTime, "", "", false},
	{"s", "", shortPrefix, dimTime, "", "", false},
	{"minute", "minutes", longPrefix, dimTime, "60", "", false},
	{"min", "mins", shortPrefix, dimTime, "60", "", false},
	{"hour", "hours", longPrefix, dimTime, "3600", "", false},
	{"hr", "hrs", shortPrefix, dimTime, "3600", "", false},
	{"h", "h", shortPrefix, dimTime, "3600", "", false},
	{"day", "days", longPrefix, dimTime, "86400", "", false},
	{"week", "weeks", longPrefix, dimTime, "604800", "", false},

	{"meter", "meters", longPrefix, dimLength, "", "", false},
	{"metre", "metres", longPrefix, dimLength, "", "", false},
	{"m", "", shortPrefix, dimLength, "", "", false},
	{"inch", "inches", noPrefix, dimLength, "0.0254", "", false},
	{"foot", "feet", longPrefix, dimLength, "0.3048", "", false},
	{"ft", "", noPrefix, dimLength, "0.3048", "", false},
	{"yard", "yards", longPrefix, dimLength, "0.9144", "", false},
	{"yd", "", noPrefix, dimLength, "0.9144", "", false},
	{"mile", "miles", longPrefix, dimLength, "1609.344", "", false},
	{"mi", "", noPrefix, dimLength, "1609.344", "", false},

	{"kilogram", "kilograms", longPrefix, dimMass, "", "", false},
	{"gram", "grams", longPrefix, dimMass, "1/1000", "", false},
	{"g", "", shortPrefix, dimMass, "1/1000", "", false},
	{"tonne", "tonnes", longPrefix, dimMass, "1000", "", false},
	{"pound", "pounds", noPrefix, dimMass, "0.45359237", "", false},
	{"lb", "lbs", noPrefix, dimMass, "0.45359237", "", false},
	{"ounce", "ounces", noPrefix, dimMass, "0.028349523125", "", false},
	{"oz", "", noPrefix, dimMass, "0.028349523125", "", false},

	{"kelvin", "", longPrefix, dimTemperature, "", "", false},
	{"K", "", shortPrefix, dimTemperature, "", "", false},
	{"celsius", "", longPrefix, dimTemperature, "", "273.15", false},
	{"°C", "", noPrefix, dimTemperature, "", "273.15", false},
	{"fahrenheit", "", longPrefix, dimTemperature, "5/9", "45967/180", false},
	{"°F", "", noPrefix, dimTemperature, "5/9", "45967/180", false},

	{"bit", "bits", longPrefix, dimData, "", "", false},
	{"b", "", shortPrefix, dimData, "", "", false},
	{"byte", "bytes", longPrefix, dimData, "8", "", false},
	{"B", "", shortPrefix, dimData, "8", "", false},
	{"bps", "", shortPrefix, dimData, "", "", true},
	// fend reads `Bps` as bits; the Rust app defined it as bytes per second,
	// so `8 Mbps to MBps` is 1 MBps.
	{"Bps", "", shortPrefix, dimData, "8", "", true},
}

// numberWords are names of plain numbers: `20k` is 20000.
var numberWords = map[string]string{
	"k": "1000", "M": "1000000", "G": "1000000000", "T": "1000000000000",
	"dozen": "12", "hundred": "100", "thousand": "1000", "million": "1000000",
	"billion": "1000000000", "trillion": "1000000000000",
}

type prefixDef struct{ name, scale string }

// fend-core's prefixes, in its order: a name that matches only in another
// case takes the first one.
var shortPrefixes = []prefixDef{
	{"Ki", "1024"}, {"Mi", "1048576"}, {"Gi", "1073741824"}, {"Ti", "1099511627776"},
	{"Pi", "1125899906842624"}, {"Ei", "1152921504606846976"},
	{"Q", "1e30"}, {"R", "1e27"}, {"Y", "1e24"}, {"Z", "1e21"}, {"E", "1e18"}, {"P", "1e15"},
	{"T", "1e12"}, {"G", "1e9"}, {"M", "1e6"}, {"k", "1e3"}, {"h", "1e2"}, {"da", "1e1"},
	{"d", "1e-1"}, {"c", "1e-2"}, {"m", "1e-3"}, {"u", "1e-6"}, {"µ", "1e-6"}, {"μ", "1e-6"},
	{"n", "1e-9"}, {"p", "1e-12"}, {"f", "1e-15"}, {"a", "1e-18"}, {"z", "1e-21"},
	{"y", "1e-24"}, {"r", "1e-27"}, {"q", "1e-30"},
}

var longPrefixes = []prefixDef{
	{"quetta", "1e30"}, {"ronna", "1e27"}, {"yotta", "1e24"}, {"zetta", "1e21"},
	{"exa", "1e18"}, {"peta", "1e15"}, {"tera", "1e12"}, {"giga", "1e9"}, {"mega", "1e6"},
	{"kilo", "1e3"}, {"hecto", "1e2"}, {"deca", "1e1"}, {"deka", "1e1"}, {"deci", "1e-1"},
	{"centi", "1e-2"}, {"milli", "1e-3"}, {"micro", "1e-6"}, {"nano", "1e-9"},
	{"pico", "1e-12"}, {"femto", "1e-15"}, {"atto", "1e-18"}, {"zepto", "1e-21"},
	{"yocto", "1e-24"}, {"ronto", "1e-27"}, {"quecto", "1e-30"},
	{"kibi", "1024"}, {"mebi", "1048576"}, {"gibi", "1073741824"}, {"tebi", "1099511627776"},
}

// currencyCodes are the ISO 4217 codes that fend-core knows. A query may name
// any of them; the ECB table has rates for about 30.
var currencyCodes = strings.Fields(`
	AED AFN ALL AMD ANG AOA ARS AUD AWG AZN BAM BBD BDT BGN BHD BIF BMD BND BOB BOV
	BRL BSD BTN BWP BYN BZD CAD CDF CHE CHF CHW CLF CLP CNY COP COU CRC CUC CUP CVE
	CZK DJF DKK DOP DZD EGP ERN ETB EUR FJD FKP GBP GEL GHS GIP GMD GNF GTQ GYD HKD
	HNL HRK HTG HUF IDR ILS INR IQD IRR ISK JMD JOD JPY KES KGS KHR KMF KPW KRW KWD
	KYD KZT LAK LBP LKR LRD LSL LYD MAD MDL MGA MKD MMK MNT MOP MRU MUR MVR MWK MXN
	MXV MYR MZN NAD NGN NIO NOK NPR NZD OMR PAB PEN PGK PHP PKR PLN PYG QAR RON RSD
	RUB RWF SAR SBD SCR SDG SEK SGD SHP SLE SLL SOS SRD SSP STN SVC SYP SZL THB TJS
	TMT TND TOP TRY TTD TWD TZS UAH UGX USD USN UYI UYU UYW UZS VED VES VND VUV WST
	XAF XAG XAU XBA XBB XBC XBD XCD XDR XOF XPD XPF XPT XSU XTS XUA XXX YER ZAR ZMW ZWL`)

type currencyName struct {
	singular, plural, code string
	// share is the part of one unit of code: a cent is 1/100 of a dollar.
	share string
}

// currencyNames are fend-core's signs and names of currencies. `$`, `£`, and
// `¥` show before the amount.
var currencyNames = []currencyName{
	{"dollar", "dollars", "USD", ""}, {"cent", "cents", "USD", "1/100"},
	{"US$", "US$", "USD", ""}, {"$", "$", "USD", ""}, {"euro", "euros", "EUR", ""},
	{"€", "€", "EUR", ""}, {"£", "£", "GBP", ""}, {"¥", "¥", "JPY", ""},
	{"AU$", "AU$", "AUD", ""}, {"HK$", "HK$", "HKD", ""}, {"NZ$", "NZ$", "NZD", ""},
	{"zł", "zł", "PLN", ""}, {"zl", "zl", "PLN", ""}, {"₺", "₺", "TRY", ""},
	{"TL", "TL", "TRY", ""},
}

// yuanNames are names that people use for the yuan, where fend knows only
// the ISO code, in any case. `RMB` is not an ISO code, but people write it.
var yuanNames = []string{"rmb", "renminbi", "yuan"}

var percentUnit = unit{singular: "%", plural: "%", scale: big.NewRat(1, 100), percent: true}

func rat(text string) *big.Rat {
	if text == "" {
		return big.NewRat(1, 1)
	}
	r, ok := new(big.Rat).SetString(text)
	if !ok {
		panic("calc: bad rational " + text)
	}
	return r
}

func (d unitDef) unit(prefix prefixDef) unit {
	u := unit{
		singular: prefix.name + d.singular,
		plural:   prefix.name + d.plural,
		scale:    rat(d.scale),
	}
	if d.plural == "" {
		u.plural = u.singular
	}
	if prefix.scale != "" {
		u.scale.Mul(u.scale, rat(prefix.scale))
	}
	if d.offset != "" {
		u.offset = rat(d.offset)
	}
	u.dims[d.dim] = 1
	if d.perSecond {
		u.dims[dimTime] = -1
	}
	return u
}

// lookupUnit finds a unit as fend-core does: first by its exact name, then
// in any case, each time also after a prefix. Currencies come first. ok is
// false for a name that is not a unit.
func lookupUnit(name string, rates *rateSource) (u unit, ok bool, err error) {
	for _, exact := range []bool{true, false} {
		if code, share, singular, plural, ok := currency(name, exact); ok {
			scale, err := rates.euros(code)
			if err != nil {
				return unit{}, false, err
			}
			scale.Mul(scale, rat(share))
			u := unit{singular: singular, plural: plural, scale: scale, currency: code}
			u.dims[dimMoney] = 1
			return u, true, nil
		}
		if u, ok := namedUnit(name, exact); ok {
			return u, true, nil
		}
	}
	return unit{}, false, nil
}

func currency(name string, exact bool) (code, share, singular, plural string, ok bool) {
	key := name
	if !exact {
		key = strings.ToUpper(name)
	}
	for _, c := range currencyCodes {
		if c == key {
			return c, "", c, c, true
		}
	}
	for _, c := range currencyNames {
		if c.singular == name || c.plural == name || !exact && (asciiEqualFold(c.singular, name) || asciiEqualFold(c.plural, name)) {
			return c.code, c.share, c.singular, c.plural, true
		}
	}
	for _, yuan := range yuanNames {
		if asciiEqualFold(name, yuan) {
			return "CNY", "", "CNY", "CNY", true
		}
	}
	return "", "", "", "", false
}

func namedUnit(name string, exact bool) (unit, bool) {
	if d, ok := findUnit(units, name, exact); ok {
		return d.unit(prefixDef{}), true
	}
	for _, p := range shortPrefixes {
		if rest, ok := cutPrefix(name, p.name, exact); ok {
			if d, ok := findUnit(units, rest, exact); ok && d.prefixes == shortPrefix {
				return d.unit(p), true
			}
		}
	}
	for _, p := range longPrefixes {
		if rest, ok := cutPrefix(name, p.name, exact); ok {
			if d, ok := findUnit(units, rest, exact); ok && d.prefixes == longPrefix {
				return d.unit(p), true
			}
		}
	}
	return unit{}, false
}

// findUnit matches name exactly, or else, when exact is false, in any case
// when only one unit matches that way.
func findUnit(defs []unitDef, name string, exact bool) (unitDef, bool) {
	var found []unitDef
	for _, d := range defs {
		if d.singular == name || d.plural == name {
			return d, true
		}
		if !exact && (asciiEqualFold(d.singular, name) || asciiEqualFold(d.plural, name)) {
			found = append(found, d)
		}
	}
	if len(found) == 1 {
		return found[0], true
	}
	return unitDef{}, false
}

func cutPrefix(name, prefix string, exact bool) (string, bool) {
	if len(name) <= len(prefix) {
		return "", false
	}
	if exact {
		return strings.CutPrefix(name, prefix)
	}
	if asciiEqualFold(name[:len(prefix)], prefix) {
		return name[len(prefix):], true
	}
	return "", false
}

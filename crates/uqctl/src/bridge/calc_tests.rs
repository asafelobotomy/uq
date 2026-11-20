use super::*;

fn ev(q: &str) -> Option<String> {
    evaluate_with(q, None)
}

#[test]
fn precedence() {
    assert_eq!(ev("= 2+3*4").as_deref(), Some("14"));
    assert_eq!(ev("2^3^2").as_deref(), Some("512"));
    assert_eq!(ev("-2^2").as_deref(), Some("-4"));
    assert_eq!(ev("2**10").as_deref(), Some("1024"));
    assert_eq!(ev("+3 - -2").as_deref(), Some("5"));
}

#[test]
fn percent() {
    assert_eq!(ev("18% of 2450").as_deref(), Some("441"));
    assert_eq!(ev("50%").as_deref(), Some("0.5"));
    // Like a desk calculator: adding a percentage adds that share of the left side.
    assert_eq!(ev("2450 + 18%").as_deref(), Some("2891"));
    assert_eq!(ev("80 - 25%").as_deref(), Some("60"));
    assert_eq!(ev("10 * 50%").as_deref(), Some("5"));
}

#[test]
fn remainder() {
    assert_eq!(ev("7 % 3").as_deref(), Some("1"));
    assert_eq!(ev("7%3").as_deref(), Some("1"));
    assert_eq!(ev("17 mod 5").as_deref(), Some("2"));
    assert_eq!(ev("1 mod 0"), None);
}

#[test]
fn functions() {
    assert_eq!(ev("sqrt(16)").as_deref(), Some("4"));
    assert_eq!(ev("round(2.5)").as_deref(), Some("3"));
    assert_eq!(ev("ln(e)").as_deref(), Some("1"));
    assert_eq!(ev("log2(1024)").as_deref(), Some("10"));
    assert_eq!(ev("cbrt(27)").as_deref(), Some("3"));
    assert_eq!(ev("floor(2.7)+ceil(2.1)").as_deref(), Some("5"));
    assert_eq!(ev("exp(0)").as_deref(), Some("1"));
    assert_eq!(ev("sqrt (9)").as_deref(), Some("3"));
}

#[test]
fn degrees_and_constants() {
    assert_eq!(ev("sin(30°)").as_deref(), Some("0.5"));
    assert_eq!(ev("cos(180°)").as_deref(), Some("-1"));
    assert_eq!(ev("=tau/pi").as_deref(), Some("2"));
    assert_eq!(ev("=2π").as_deref(), Some("6.28318530718"));
    assert_eq!(ev("=phi").as_deref(), Some("1.61803398875"));
}

#[test]
fn implicit_multiplication() {
    assert_eq!(ev("=2pi").as_deref(), Some("6.28318530718"));
    assert_eq!(ev("3(1+1)").as_deref(), Some("6"));
    assert_eq!(ev("(1+1)(2+2)").as_deref(), Some("8"));
    assert_eq!(ev("2sqrt(9)").as_deref(), Some("6"));
    assert_eq!(ev("=2e").as_deref(), Some("5.43656365692"));
    // Two plain numbers are not a product.
    assert_eq!(ev("=2 3"), None);
}

#[test]
fn factorial() {
    assert_eq!(ev("5!").as_deref(), Some("120"));
    assert_eq!(ev("=18!").as_deref(), Some("6402373705728000"));
    assert_eq!(ev("=20!").as_deref(), Some("2.43290200818e18"));
    assert_eq!(ev("(1+2)!").as_deref(), Some("6"));
    assert_eq!(ev("-3!").as_deref(), Some("-6"));
    assert_eq!(ev("2.5!"), None);
    assert_eq!(ev("171!"), None);
}

#[test]
fn number_forms() {
    assert_eq!(ev("1,234,567 + 1").as_deref(), Some("1234568"));
    assert_eq!(ev("1_000*3").as_deref(), Some("3000"));
    assert_eq!(ev("1e3+1").as_deref(), Some("1001"));
    assert_eq!(ev("2.5E-3*2").as_deref(), Some("0.005"));
    assert_eq!(ev("0xff+1").as_deref(), Some("256"));
    assert_eq!(ev("0b101*2").as_deref(), Some("10"));
    assert_eq!(ev("=0o17").as_deref(), Some("15"));
    assert_eq!(ev("12 × 3 ÷ 4 − 1").as_deref(), Some("8"));
    // A comma that isn't a thousands separator is an error, not a silent guess.
    assert_eq!(ev("1,23+1"), None);
}

#[test]
fn bases() {
    assert_eq!(ev("255 in hex").as_deref(), Some("0xff"));
    assert_eq!(ev("=10 to bin").as_deref(), Some("0b1010"));
    assert_eq!(ev("8*8 as octal").as_deref(), Some("0o100"));
    assert_eq!(ev("0xff in dec").as_deref(), Some("255"));
    assert_eq!(ev("-16 in hex").as_deref(), Some("-0x10"));
    assert_eq!(ev("1.5 in hex"), None);
    assert!(applies("255 in hex"));
    assert!(applies("0xff"));
}

#[test]
fn ans() {
    assert_eq!(evaluate_with("ans*2", Some(21.0)).as_deref(), Some("42"));
    assert_eq!(evaluate_with("2ans", Some(4.0)).as_deref(), Some("8"));
    assert_eq!(evaluate_with("ans+1", None), None);
}

#[test]
fn big_integers_print_in_full() {
    assert_eq!(ev("2^53").as_deref(), Some("9007199254740992"));
    assert_eq!(ev("2^60").as_deref(), Some("1.15292150461e18"));
    assert_eq!(ev("0x20000000000001"), None);
}

#[test]
fn bare_number_not_calc() {
    assert!(!applies("42"));
    assert!(applies("=42"));
    assert!(!applies("-5"));
    assert!(!applies("e"));
    assert!(!applies("firefox"));
    assert!(applies("2+2"));
    assert!(applies("sqrt(4)"));
    assert!(applies("5!"));
    assert!(applies("17 mod 5"));
    assert!(applies("6×7"));
}

#[test]
fn limits() {
    let long = format!("{}1", "1+".repeat(128));
    assert_eq!(long.chars().count(), 257);
    assert_eq!(ev(&long), None);
    let nest = |n: usize| format!("{}1{}", "(".repeat(n), ")".repeat(n));
    assert_eq!(ev(&nest(33)), None);
    assert_eq!(ev(&nest(32)).as_deref(), Some("1"));
}

#[test]
fn errors() {
    assert_eq!(ev("1/0"), None);
    assert_eq!(ev("2+"), None);
    assert_eq!(ev("foo(1)"), None);
    assert_eq!(ev("()"), None);
    assert_eq!(ev("sqrt(-1)"), None);
    assert_eq!(ev("asin(2)"), None);
    assert_eq!(ev("sqrt 4"), None);
    assert_eq!(ev("2 ^ * 3"), None);
}

#[test]
fn no_code_execution() {
    assert_eq!(ev("= system(\"x\")"), None);
}

#[test]
fn rounding() {
    assert_eq!(ev("0.1+0.2").as_deref(), Some("0.3"));
    assert_eq!(ev("1/3").as_deref(), Some("0.333333333333"));
}

#[test]
fn remember_sets_ans() {
    assert_eq!(remember("= 6*7").as_deref(), Some("42"));
    assert_eq!(evaluate("ans/2").as_deref(), Some("21"));
    assert_eq!(remember("1/0"), None);
    assert_eq!(evaluate("ans").as_deref(), Some("42"));
}

#[test]
fn item_shape() {
    let it = item("= 1+1").unwrap();
    assert_eq!((it.id.as_str(), it.title.as_str()), ("calc", "2"));
    assert_eq!(it.subtitle, "1+1");
    assert_eq!(it.actions, vec!["copy"]);
}

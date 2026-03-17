use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options, parse_document};

fn main() {
    let input = r#"1. ## Definitions {#definitions}

    1. **Claim** means any and all claims.

    2. **Person** means a natural person.

2. ## Employer's Obligations {#employer-obligations}

    1. The **Employer** agrees to pay [the Payment][ref-1].

    2. The **Employer** shall pay within [the Timeframe][ref-2]. {#payment-timeframe}

        1. completing a handover; and

        2. returning all property.

            1. including laptops; and

            2. including phones.

[ref-1]: #schedule "AU $10,000"
[ref-2]: #schedule ""
"#;

    let arena = Arena::new();
    let opts = Options::default();
    let root = parse_document(&arena, input, &opts);

    dump(root, 0);
}

fn dump<'a>(node: &'a AstNode<'a>, depth: usize) {
    let indent = "  ".repeat(depth);
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Document => println!("{}Document", indent),
        NodeValue::List(l) => println!(
            "{}List(type={}, start={})",
            indent, l.list_type as u8, l.start
        ),
        NodeValue::Item(l) => println!("{}Item", indent),
        NodeValue::Heading(h) => println!("{}Heading(level={})", indent, h.level),
        NodeValue::Paragraph => println!("{}Paragraph", indent),
        NodeValue::Text(t) => println!("{}Text({:?})", indent, t),
        NodeValue::Strong => println!("{}Strong", indent),
        NodeValue::Emph => println!("{}Emph", indent),
        NodeValue::Link(l) => println!("{}Link(url={:?}, title={:?})", indent, l.url, l.title),
        NodeValue::SoftBreak => println!("{}SoftBreak", indent),
        NodeValue::LineBreak => println!("{}LineBreak", indent),
        NodeValue::Code(c) => println!("{}Code({:?})", indent, c.literal),
        NodeValue::BlockQuote => println!("{}BlockQuote", indent),
        NodeValue::Table(_) => println!("{}Table", indent),
        NodeValue::TableRow(_) => println!("{}TableRow", indent),
        NodeValue::TableCell => println!("{}TableCell", indent),
        _ => println!("{}{:?}", indent, std::mem::discriminant(&data.value)),
    }
    drop(data);
    for child in node.children() {
        dump(child, depth + 1);
    }
}

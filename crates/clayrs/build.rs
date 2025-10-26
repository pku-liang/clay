fn main() {
    lalrpop::process_src().unwrap();
    println!("cargo:rerun-if-changed=./src/ast/grammar.lalrpop");
}
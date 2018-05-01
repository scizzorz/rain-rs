use bincode::serialize;
use bincode;
use blake2::Blake2b;
// use blake2::Digest;
use blake2::digest::Input;
use blake2::digest::VariableOutput;
use code::Data;
use code::Instr;
use codemap::CodeMap;
use codemap::File;
use compiler::CompileErrorKind;
use compiler::Compiler;
use lexer;
use parser::ParseErrorKind;
use parser;
use semck::CheckErrorKind;
use semck::SemChecker;
use std::fs;
use std::io::Read;
use std::io;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub enum ModuleErrorKind {
  CheckError(CheckErrorKind),
  ParseError(ParseErrorKind),
  CompileError(CompileErrorKind),
  IOError(io::Error),
  BincodeError(bincode::Error),
}

#[derive(Serialize, Deserialize)]
pub struct Module {
  lex_hash: [u8; 8],
  pub code: Vec<Instr>,
  pub consts: Vec<Data>,
}

impl Module {
  pub fn from_string(map: &mut CodeMap, chunk: &str) -> Result<Module, ModuleErrorKind> {
    let file = map.add_file(String::from("_anon"), chunk.to_string());
    Module::new(map, file)
  }

  pub fn from_file(map: &mut CodeMap, filename: &str) -> Result<Module, ModuleErrorKind> {
    let path = Path::new(&filename);
    let mut fs_file = match fs::File::open(path) {
      Ok(file) => file,
      Err(why) => return Err(ModuleErrorKind::IOError(why)),
    };

    let mut contents = String::new();
    let file = match fs_file.read_to_string(&mut contents) {
      Ok(_) => map.add_file(filename.to_string(), contents.to_string()),
      Err(why) => return Err(ModuleErrorKind::IOError(why)),
    };

    Module::new(map, file)
  }

  pub fn new(map: &CodeMap, file: Arc<File>) -> Result<Module, ModuleErrorKind> {
    let tokens = lexer::lex(&file);
    let hashable_tokens: Vec<_> = tokens.iter().map(|x| x.node.clone()).collect();
    let token_bytes = serialize(&hashable_tokens);

    let lex_hash = match token_bytes {
      Ok(x) => {
        // these unwraps should be safe because the output size is hardcoded
        let mut hasher = Blake2b::new(8).unwrap();
        hasher.process(&x);
        let mut buf = [0u8; 8];
        hasher.variable_result(&mut buf).unwrap();
        buf
      },
      Err(why) => return Err(ModuleErrorKind::BincodeError(why)),
    };

    let mut ast = match parser::parse(tokens) {
      Ok(root) => root,
      Err(why) => return Err(ModuleErrorKind::ParseError(why)),
    };

    let mut ck = SemChecker::new();
    match ck.check(&mut ast) {
      Err(why) => return Err(ModuleErrorKind::CheckError(why)),
      _ => {}
    }

    let mut compiler = Compiler::new();
    match compiler.compile(&ast) {
      Err(why) => return Err(ModuleErrorKind::CompileError(why)),
      _ => {}
    }

    Ok(Module {
      lex_hash,
      code: compiler.get_instrs(),
      consts: compiler.get_consts(),
    })
  }
}

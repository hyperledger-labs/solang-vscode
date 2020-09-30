use jsonrpc_core::Result;
use serde_json::Value;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use solang::file_cache::FileCache;
use solang::parse_and_resolve;
use solang::Target;

use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use solang::sema::*;

use std::path::PathBuf;

use solang::*;

use solang::sema::ast::*;

use solang::parser::pt;

use solang::sema::ast::Expression::*;

#[derive(Debug, Default)]
pub struct Backend {
    state: Vec<usize>,
}

impl Backend {
    // Calculate the line and coloumn from the Loc offset recieved from the parser
    // Do a linear search till the correct offset location is matched
    fn file_offset_to_line_column(data: &str, loc: usize) -> (usize, usize) {
        let mut line_no = 0;
        let mut past_ch = 0;

        for (ind, c) in data.char_indices() {
            if c == '\n' {
                if ind == loc {
                    break;
                } else {
                    past_ch = ind + 1;
                    line_no += 1;
                }
            }
            if ind == loc {
                break;
            }
        }

        (line_no, loc - past_ch)
    }

    // Convert the diagnostic messages recieved from the solang to lsp diagnostics types.
    // Returns a vector of diagnostic messages for the client.
    fn convert_to_diagnostics(ns: ast::Namespace, filecache: &mut FileCache) -> Vec<Diagnostic> {
        let mut diagnostics_vec: Vec<Diagnostic> = Vec::new();

        for diag in ns.diagnostics {
            let pos = diag.pos.unwrap();

            let diagnostic = &diag;

            let sev = match diagnostic.level {
                ast::Level::Info => DiagnosticSeverity::Information,
                ast::Level::Warning => DiagnosticSeverity::Warning,
                ast::Level::Error => DiagnosticSeverity::Error,
                ast::Level::Debug => continue,
            };

            let fl = &ns.files[pos.0];

            let file_cont = filecache.get_file_contents(fl.as_str());

            let l1 = Backend::file_offset_to_line_column(&file_cont.as_str(), pos.1);

            let l2 = Backend::file_offset_to_line_column(&file_cont.as_str(), pos.2);

            let p1 = Position::new(l1.0 as u64, l1.1 as u64);

            let p2 = Position::new(l2.0 as u64, l2.1 as u64);

            let range = Range::new(p1, p2);

            let message_slice = &diag.message[..];

            diagnostics_vec.push(Diagnostic {
                range,
                message: message_slice.to_string(),
                severity: Some(sev),
                source: Some("solidity".to_string()),
                code: None,
                related_information: None,
                tags: None,
            });
        }

        diagnostics_vec
    }

    // Constructs the function type message which is returned as a String
    fn construct_fnc(fnc_ty: &pt::FunctionTy) -> String {
        let msg;
        match fnc_ty {
            pt::FunctionTy::Constructor => {
                msg = String::from("Constructor");
            }
            pt::FunctionTy::Function => {
                msg = String::from("Function");
            }
            pt::FunctionTy::Fallback => {
                msg = String::from("Fallback");
            }
            pt::FunctionTy::Receive => {
                msg = String::from("Recieve");
            }
            pt::FunctionTy::Modifier => {
                msg = String::from("Modifier");
            }
        }
        msg
    }

    // Constructs lookup table(messages) for the given statement by traversing the
    // statements and traversing inside the contents of the statements.
    fn construct_stmt(
        stmt: &Statement,
        lookup_tbl: &mut Vec<(u64, u64, String)>,
        symtab: &sema::symtable::Symtable,
        ns: &ast::Namespace,
    ) {
        match stmt {
            Statement::VariableDecl(_locs, _, _param, expr) => {
                if let Some(exp) = expr {
                    Backend::construct_expr(exp, lookup_tbl, symtab, ns);
                }
                let msg = (_param.ty).to_string(ns);
                lookup_tbl.push((_param.loc.0 as u64, _param.loc.1 as u64, msg));
            }
            Statement::If(_locs, _, expr, stat1, stat2) => {
                //let _if_msg = String::from("If(...)");
                Backend::construct_expr(expr, lookup_tbl, symtab, ns);
                for st1 in stat1 {
                    Backend::construct_stmt(st1, lookup_tbl, symtab, ns);
                }
                for st2 in stat2 {
                    Backend::construct_stmt(st2, lookup_tbl, symtab, ns);
                }
            }
            Statement::While(_locs, _blval, expr, stat1) => {
                Backend::construct_expr(expr, lookup_tbl, symtab, ns);
                for st1 in stat1 {
                    Backend::construct_stmt(st1, lookup_tbl, symtab, ns);
                }
            }
            Statement::For {
                loc: _,
                reachable: _,
                init,
                cond,
                next,
                body,
            } => {
                if let Some(exp) = cond {
                    Backend::construct_expr(exp, lookup_tbl, symtab, ns);
                }
                for stat in init {
                    Backend::construct_stmt(stat, lookup_tbl, symtab, ns);
                }
                for stat in next {
                    Backend::construct_stmt(stat, lookup_tbl, symtab, ns);
                }
                for stat in body {
                    Backend::construct_stmt(stat, lookup_tbl, symtab, ns);
                }
            }
            Statement::DoWhile(_locs, _blval, stat1, expr) => {
                Backend::construct_expr(expr, lookup_tbl, symtab, ns);
                for st1 in stat1 {
                    Backend::construct_stmt(st1, lookup_tbl, symtab, ns);
                }
            }
            Statement::Expression(_locs, _, expr) => {
                Backend::construct_expr(expr, lookup_tbl, symtab, ns);
            }
            Statement::Delete(_locs, _typ, expr) => {
                Backend::construct_expr(expr, lookup_tbl, symtab, ns);
            }
            Statement::Destructure(_locs, _vecdestrfield, expr) => {
                Backend::construct_expr(expr, lookup_tbl, symtab, ns);
                for vecstr in _vecdestrfield {
                    match vecstr {
                        DestructureField::Expression(expr) => {
                            Backend::construct_expr( expr, lookup_tbl, symtab, ns);
                        }
                        _ => continue
                    }
                }
            }
            Statement::Continue(_locs) => {}
            Statement::Break(_) => {}
            Statement::Return(_locs, expr) => {
                for expp in expr {
                    Backend::construct_expr(expp, lookup_tbl, symtab, ns);
                }
            }
            Statement::Emit {
                loc:_,
                event_no: _,
                args,
            } => {
                for arg in args {
                    Backend::construct_expr(arg, lookup_tbl, symtab, ns);
                }
            }
            Statement::TryCatch {
                loc: _,
                reachable: _,
                expr,
                returns: _,
                ok_stmt,
                error,
                catch_param: _,
                catch_param_pos: _,
                catch_stmt,
            } => {
                Backend::construct_expr(expr, lookup_tbl, symtab, ns);
                for vecstmt in catch_stmt {
                    Backend::construct_stmt(vecstmt, lookup_tbl, symtab, ns);
                }
                for vecstmt in ok_stmt {
                    Backend::construct_stmt(vecstmt, lookup_tbl, symtab, ns);
                }
                if let Some(okstmt) = error {
                        for stmts in &okstmt.2 {
                            Backend::construct_stmt( &stmts, lookup_tbl, symtab, ns);
                        }
                    }
                }
            Statement::Underscore(_loc) => {}
        }
    }

    // Constructs lookup table(messages) by traversing over the expressions and storing
    // the respective expression type messages in the table.
    fn construct_expr(
        expr: &Expression,
        lookup_tbl: &mut Vec<(u64, u64, String)>,
        symtab: &sema::symtable::Symtable,
        ns: &ast::Namespace,
    ) {
        match expr {
            FunctionArg(locs, typ, _sample_sz) => {
                let msg = format!("function arg {}", typ.to_string(ns));
                lookup_tbl.push((locs.1 as u64, locs.2 as u64, msg));
            }

            // Variable types expression
            BoolLiteral(locs, vl) => {
                let msg = format!("(bool) {}", vl);
                lookup_tbl.push((locs.1 as u64, locs.2 as u64, msg));
            }
            BytesLiteral(locs, typ, _vec_lst) => {
                let msg = format!("({})", typ.to_string(ns));
                lookup_tbl.push((locs.1 as u64, locs.2 as u64, msg));
            }
            CodeLiteral(locs, _val, _) => {
                let msg = format!("({})", _val);
                lookup_tbl.push((locs.1 as u64, locs.2 as u64, msg));
            }
            NumberLiteral(locs, typ, _bgit) => {
                let msg = format!("({})", typ.to_string(ns));
                lookup_tbl.push((locs.1 as u64, locs.2 as u64, msg));
            }
            StructLiteral(_locs, _typ, expr) => {
                for expp in expr {
                    Backend::construct_expr(expp, lookup_tbl, symtab, ns);
                }
            }
            ArrayLiteral(_locs, _, _arr, expr) => {
                for expp in expr {
                    Backend::construct_expr(expp, lookup_tbl, symtab, ns);
                }
            }
            ConstArrayLiteral(_locs, _, _arr, expr) => {
                for expp in expr {
                    Backend::construct_expr(expp, lookup_tbl, symtab, ns);
                }
            }

            // Arithmetic expression
            Add(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            Subtract(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            Multiply(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            UDivide(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            SDivide(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            UModulo(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            SModulo(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            Power(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }

            // Bitwise expresion
            BitwiseOr(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            BitwiseAnd(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            BitwiseXor(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            ShiftLeft(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            ShiftRight(_locs, _typ, expr1, expr2, _bl) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }

            // Variable expression
            Variable(locs, typ, _val) => {
                let msg = format!("({})", typ.to_string(ns));
                lookup_tbl.push((locs.1 as u64, locs.2 as u64, msg));
            }
            ConstantVariable(locs, typ, _val1, _val2) => {

                let msg = format!("constant ({})", typ.to_string(ns));
                lookup_tbl.push((locs.1 as u64, locs.2 as u64, msg));
            }
            StorageVariable(locs, typ, _val1, _val2) => {
                let msg = format!("({})", typ.to_string(ns));
                lookup_tbl.push((locs.1 as u64, locs.2 as u64, msg));
            }

            // Load expression
            Load(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            StorageLoad(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            ZeroExt(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            SignExt(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            Trunc(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            Cast(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            BytesCast(_loc, _typ1, _typ2, expr) => {
                Backend::construct_expr(expr, lookup_tbl, symtab, ns);
            }

            //Increment-Decrement expression
            PreIncrement(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            PreDecrement(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            PostIncrement(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            PostDecrement(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            Assign(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }

            // Compare expression
            UMore(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            ULess(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            UMoreEqual(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            ULessEqual(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            SMore(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            SLess(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            SMoreEqual(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            SLessEqual(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            Equal(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            NotEqual(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }

            Not(_locs, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            Complement(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            UnaryMinus(_locs, _typ, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }

            Ternary(_locs, _typ, expr1, expr2, expr3) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr3, lookup_tbl, symtab, ns);
            }

            ArraySubscript(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }

            StructMember(_locs, _typ, expr1, _val) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }

            // Array operation expression
            AllocDynamicArray(_locs, _typ, expr1, _valvec) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            DynamicArrayLength(_locs, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            DynamicArraySubscript(_locs, _typ, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            DynamicArrayPush(_locs, expr1, _typ, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            DynamicArrayPop(_locs, expr1, _typ) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            StorageBytesSubscript(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            StorageBytesPush(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            StorageBytesPop(_locs, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }
            StorageBytesLength(_locs, expr1) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
            }

            //String operations expression
            StringCompare(_locs, _strloc1, _strloc2) => {
                if let StringLocation::RunTime(expr1) = _strloc1 {
                    Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                }
                if let StringLocation::RunTime(expr2) = _strloc1 {
                    Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
                }
            }
            StringConcat(_locs, _typ, _strloc1, _strloc2) => {
                if let StringLocation::RunTime(expr1) = _strloc1 {
                    Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                }
                if let StringLocation::RunTime(expr2) = _strloc1 {
                    Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
                }
            }

            Or(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }
            And(_locs, expr1, expr2) => {
                Backend::construct_expr(expr1, lookup_tbl, symtab, ns);
                Backend::construct_expr(expr2, lookup_tbl, symtab, ns);
            }

            // Function call expression
            InternalFunctionCall {
                loc: _,
                returns: _,
                contract_no: _,
                function_no: _,
                signature: _,
                args,
            } => {
                for expp in args {
                    Backend::construct_expr(expp, lookup_tbl, symtab, ns);
                }
            }
            ExternalFunctionCall {
                loc: _,
                returns: _,
                contract_no: _,
                function_no: _,
                address,
                args,
                value,
                gas,
            } => {
                Backend::construct_expr(address, lookup_tbl, symtab, ns);
                for expp in args {
                    Backend::construct_expr(expp, lookup_tbl, symtab, ns);
                }

                Backend::construct_expr(value, lookup_tbl, symtab, ns);
                Backend::construct_expr(gas, lookup_tbl, symtab, ns);
            }
            ExternalFunctionCallRaw {
                loc: _,
                ty: _,
                address,
                args,
                value,
                gas,
            } => {
                Backend::construct_expr(args, lookup_tbl, symtab, ns);
                Backend::construct_expr(address, lookup_tbl, symtab, ns);
                Backend::construct_expr(value, lookup_tbl, symtab, ns);
                Backend::construct_expr(gas, lookup_tbl, symtab, ns);
            }
            Constructor {
                loc: _,
                contract_no: _,
                constructor_no: _,
                args,
                gas,
                value,
                salt,
            } => {
                Backend::construct_expr(gas, lookup_tbl, symtab, ns);
                for expp in args {
                    Backend::construct_expr(expp, lookup_tbl, symtab, ns);
                }
                if let Some(optval) = value {
                    Backend::construct_expr(optval, lookup_tbl, symtab, ns);
                }
                if let Some(optsalt) = salt {
                    Backend::construct_expr(optsalt, lookup_tbl, symtab, ns);
                }
            }

            //Hash table operation expression
            Keccak256(_locs, _typ, expr) => {
                for expp in expr {
                    Backend::construct_expr(expp, lookup_tbl, symtab, ns);
                }
                lookup_tbl.push((
                    _locs.1 as u64,
                    _locs.2 as u64,
                    String::from("Keccak256 hash"),
                ));
            }

            ReturnData(locs) => {
                let msg = String::from("Return");
                lookup_tbl.push((locs.1 as u64, locs.2 as u64, msg));
            }
            GetAddress(locs, _typ) => {
                let msg = String::from("Get address");
                lookup_tbl.push((locs.1 as u64, locs.2 as u64, msg));
            }
            Balance(_locs, _typ, expr) => {
                Backend::construct_expr(expr, lookup_tbl, symtab, ns);
            }
            Builtin(_locs, _typ, _builtin, expr) => {
                for expp in expr {
                    Backend::construct_expr(expp, lookup_tbl, symtab, ns);
                }
            }
            List(_locs, expr) => {
                for expp in expr {
                    Backend::construct_expr(expp, lookup_tbl, symtab, ns);
                }
            }
            Poison => {}
        }
    }

    // Constructs contract fields and stores it in the lookup table.
    fn construct_cont(
        contvar: &ContractVariable,
        lookup_tbl: &mut Vec<(u64, u64, String)>,
        samptb: &sema::symtable::Symtable,
        ns: &ast::Namespace,
    ) {
        let msg_typ = &contvar.ty.to_string(ns);
        let msg = format!("{} {}", msg_typ, contvar.name);
        lookup_tbl.push((contvar.loc.1 as u64, contvar.loc.2 as u64, msg));
        if let Some(expr) = &contvar.initializer {
            Backend::construct_expr(&expr, lookup_tbl, samptb, ns);
        }
    }

    // Constructs struct fields and stores it in the lookup table.
    fn construct_strct(
        strfld: &Parameter,
        lookup_tbl: &mut Vec<(u64, u64, String)>,
        ns: &ast::Namespace,
    ) {
        let msg_typ = &strfld.ty.to_string(ns);
        let msg = format!("{} {}", msg_typ, strfld.name);
        lookup_tbl.push((strfld.loc.1 as u64, strfld.loc.2 as u64, msg));
    }

    // Traverses namespace to build messages stored in the lookup table for hover feature.
    fn traverse(ns: &ast::Namespace, lookup_tbl: &mut Vec<(u64, u64, String)>) {
        for contrct in &ns.contracts {
            for fnc in &contrct.functions {
                let fnc_msg_type = Backend::construct_fnc(&fnc.ty);
                lookup_tbl.push((fnc.loc.1 as u64, fnc.loc.1 as u64, fnc_msg_type));
                for stmt in &fnc.body {
                    Backend::construct_stmt(&stmt, lookup_tbl, &fnc.symtable, ns);
                }
            }
            for varscont in &contrct.variables {
                let samptb = symtable::Symtable::new();
                Backend::construct_cont(varscont, lookup_tbl, &samptb, ns);
            }
        }
        for strct in &ns.structs {
            for filds in &strct.fields {
                Backend::construct_strct(&filds, lookup_tbl, ns);
            }
        }
    }

    // Converts line, char position in a respective file to a file offset position of the same file.
    fn line_char_to_offset(ln: u64, chr: u64, data: &str) -> u64 {
        let mut line_no = 0;
        let mut past_ch = 0;
        let mut ofst = 0;
        for (_ind, c) in data.char_indices() {
            if line_no == ln && chr == past_ch {
                ofst = _ind;
                break;
            }
            if c == '\n' {
                line_no += 1;
                past_ch = 0;
            } else {
                past_ch += 1;
            }
        }
        ofst as u64
    }

    // Searches the respective hover message from lookup table for the given mouse pointer.
    fn get_hover_msg(offset: &u64, lookup_tbl: &[(u64, u64, String)]) -> String {
        let mut res = format!(
            "Either the code is incorrect or Feature not yet implemented for {} offset",
            offset
        );
        for (_l1, r1, msg) in lookup_tbl {
            if *_l1 <= *offset && *offset <= *r1 {
                let new_msg = &msg[..];
                res = new_msg.to_string();
                break;
            }
        }
        res
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: &Client, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::Incremental,
                )),
                hover_provider: Some(true),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    work_done_progress_options: Default::default(),
                }),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: None,
                    retrigger_characters: None,
                    work_done_progress_options: Default::default(),
                }),
                document_highlight_provider: Some(true),
                workspace_symbol_provider: Some(true),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["dummy.do_something".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                workspace: Some(WorkspaceCapability {
                    workspace_folders: Some(WorkspaceFolderCapability {
                        supported: Some(true),
                        change_notifications: Some(
                            WorkspaceFolderCapabilityChangeNotifications::Bool(true),
                        ),
                    }),
                }),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, client: &Client, _: InitializedParams) {
        client.log_message(MessageType::Info, "server initialized!");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_workspace_folders(
        &self,
        client: &Client,
        _: DidChangeWorkspaceFoldersParams,
    ) {
        client.log_message(MessageType::Info, "workspace folders changed!");
    }

    async fn did_change_configuration(&self, client: &Client, _: DidChangeConfigurationParams) {
        client.log_message(MessageType::Info, "configuration changed!");
    }

    async fn did_change_watched_files(&self, client: &Client, _: DidChangeWatchedFilesParams) {
        client.log_message(MessageType::Info, "watched files have changed!");
    }

    async fn execute_command(
        &self,
        client: &Client,
        _: ExecuteCommandParams,
    ) -> Result<Option<Value>> {
        client.log_message(MessageType::Info, "command executed!");
        Ok(None)
    }

    async fn did_open(&self, client: &Client, params: DidOpenTextDocumentParams) {
        client.log_message(MessageType::Info, "file opened!");

        let uri = params.text_document.uri;

        if let Ok(path) = uri.to_file_path() {
            let mut filecache = FileCache::new();

            let filecachepath = path.parent().unwrap();

            let tostrpath = filecachepath.to_str().unwrap();

            let mut p = PathBuf::new();

            p.push(tostrpath.to_string());

            filecache.add_import_path(p);

            let uri_string = uri.to_string();

            client.log_message(MessageType::Info, &uri_string);

            let os_str = path.file_name().unwrap();

            let ns = parse_and_resolve(os_str.to_str().unwrap(), &mut filecache, Target::Ewasm);

            let d = Backend::convert_to_diagnostics(ns, &mut filecache);

            client.publish_diagnostics(uri, d, None);
        }
    }

    async fn did_change(&self, client: &Client, params: DidChangeTextDocumentParams) {
        client.log_message(MessageType::Info, "file changed!");

        let uri = params.text_document.uri;

        if let Ok(path) = uri.to_file_path() {
            let mut filecache = FileCache::new();

            let filecachepath = path.parent().unwrap();

            let tostrpath = filecachepath.to_str().unwrap();

            let mut p = PathBuf::new();

            p.push(tostrpath.to_string());

            filecache.add_import_path(p);

            let uri_string = uri.to_string();

            client.log_message(MessageType::Info, &uri_string);

            let os_str = path.file_name().unwrap();

            let ns = parse_and_resolve(os_str.to_str().unwrap(), &mut filecache, Target::Ewasm);

            let d = Backend::convert_to_diagnostics(ns, &mut filecache);

            client.publish_diagnostics(uri, d, None);
        }
    }

    async fn did_save(&self, client: &Client, params: DidSaveTextDocumentParams) {
        client.log_message(MessageType::Info, "file saved!");

        let uri = params.text_document.uri;

        if let Ok(path) = uri.to_file_path() {
            let mut filecache = FileCache::new();

            let filecachepath = path.parent().unwrap();

            let tostrpath = filecachepath.to_str().unwrap();

            let mut p = PathBuf::new();

            p.push(tostrpath.to_string());

            filecache.add_import_path(p);

            let uri_string = uri.to_string();

            client.log_message(MessageType::Info, &uri_string);

            let os_str = path.file_name().unwrap();

            let ns = parse_and_resolve(os_str.to_str().unwrap(), &mut filecache, Target::Ewasm);

            let d = Backend::convert_to_diagnostics(ns, &mut filecache);

            client.publish_diagnostics(uri, d, None);
        }
    }

    async fn did_close(&self, client: &Client, _: DidCloseTextDocumentParams) {
        client.log_message(MessageType::Info, "file closed!");
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::Array(vec![
            CompletionItem::new_simple("Hello".to_string(), "Some detail".to_string()),
            CompletionItem::new_simple("Bye".to_string(), "More detail".to_string()),
        ])))
    }

    async fn hover(&self, hverparam: HoverParams) -> Result<Option<Hover>> {
        let txtdoc = hverparam.text_document_position_params.text_document;
        let pos = hverparam.text_document_position_params.position;

        let uri = txtdoc.uri;

        if let Ok(path) = uri.to_file_path() {
            let mut filecache = FileCache::new();

            let filecachepath = path.parent().unwrap();

            let tostrpath = filecachepath.to_str().unwrap();

            let mut p = PathBuf::new();

            p.push(tostrpath.to_string());

            filecache.add_import_path(p);

            let _uri_string = uri.to_string();

            let os_str = path.file_name().unwrap();

            let ns = parse_and_resolve(os_str.to_str().unwrap(), &mut filecache, Target::Ewasm);

            let mut lookup_tbl: Vec<(u64, u64, String)> = Vec::new();

            Backend::traverse(&ns, &mut lookup_tbl);

            let fl = &ns.files[0];

            let file_cont = filecache.get_file_contents(fl.as_str());

            let offst = Backend::line_char_to_offset(pos.line, pos.character, &file_cont.as_str()); // 0 based offset

            let msg = Backend::get_hover_msg(&offst, &lookup_tbl);

            let new_pos = (pos.line, pos.character);

            let p1 = Position::new(pos.line as u64, pos.character as u64);
            let p2 = Position::new(new_pos.0 as u64, new_pos.1 as u64);
            let new_rng = Range::new(p1, p2);

            Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(msg)),
                range: Some(new_rng),
            }))
        } else {
            Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(
                    "Failed to render hover".to_string(),
                )),
                range: None,
            }))
        }
    }
}

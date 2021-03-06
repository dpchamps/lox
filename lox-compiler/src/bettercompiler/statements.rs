use super::{CompilerError};
use crate::ast::*;
use crate::bytecode::*;
use super::compiler::Compiler;
use super::compiler::ContextType;
use crate::position::WithSpan;

pub fn compile_ast(compiler: &mut Compiler, ast: &Ast) -> Result<(), CompilerError> {
    let errors: Vec<_> = ast.iter()
        .map(|stmt| compile_stmt(compiler, stmt))
        .filter_map(Result::err)
        .collect();
    if errors.is_empty() { Ok(()) } else { Err(CompilerError::Multiple(errors)) }
}

fn compile_stmt(compiler: &mut Compiler, stmt: &Stmt) -> Result<(), CompilerError> {
    match stmt {
        Stmt::Print(ref expr) => compile_print(compiler, expr),
        Stmt::Var(ref identifier, ref expr) => compile_var_declaration(compiler, identifier.as_ref(), expr.as_ref()),
        Stmt::Block(ref stmts) => compile_block(compiler, stmts),
        Stmt::Expression(ref expr) => compile_expression_statement(compiler, expr),
        Stmt::If(ref condition, ref then_stmt, ref else_stmt) => compile_if(compiler, condition, then_stmt, else_stmt.as_ref()),
        Stmt::While(ref expr, ref stmt) => compile_while(compiler, expr, stmt),
        Stmt::Function(ref identifier, ref args, ref stmts) => compile_function(compiler, identifier, args, stmts),
        Stmt::Return(ref expr) => compile_return(compiler, expr.as_ref()),
        Stmt::Class(ref identifier, ref extends, ref stmts) => compile_class(compiler, identifier, extends.as_deref(), stmts),
    }
}

fn declare_variable(compiler: &mut Compiler, identifier: &str) -> Result<(), CompilerError> {
    if compiler.is_scoped() {
        if compiler.has_local_in_current_scope(identifier) {
            return Err(CompilerError::LocalAlreadyDefined); //TODO Don't return error, do `add_error` instead.
        }

        compiler.add_local(identifier);
    }
    Ok(())
}

fn define_variable(compiler: &mut Compiler, identifier: &str) {
    if compiler.is_scoped() {
        compiler.mark_local_initialized();
    } else {
        let constant = compiler.add_constant(identifier);
        compiler.add_instruction(Instruction::DefineGlobal(constant));
    }
}

fn compile_class(compiler: &mut Compiler, identifier: &str, _extends: Option<&str>, _stmts: &[Stmt]) -> Result<(), CompilerError> {

    declare_variable(compiler, identifier)?;
    let constant = compiler.add_constant(Constant::Class(Class{ name: identifier.to_string() }));
    compiler.add_instruction(Instruction::Class(constant));
    define_variable(compiler, identifier);

    //TODO Extends
    //TODO Methods

    Ok(())
}

fn compile_return<E: AsRef<Expr>>(compiler: &mut Compiler, expr: Option<E>) -> Result<(), CompilerError> {
    if let Some(expr) = expr {
        compile_expr(compiler, expr.as_ref())?;
    } else {
        compile_nil(compiler)?;
    }
    compiler.add_instruction(Instruction::Return);
    Ok(())
}

fn compile_function(compiler: &mut Compiler, identifier: &str, args: &Vec<Identifier>, block: &Vec<Stmt>) -> Result<(), CompilerError> {
    declare_variable(compiler, identifier)?;
    if compiler.is_scoped() {
        compiler.mark_local_initialized();
    }

    let (chunk_index, upvalues) = compiler.with_scoped_context(ContextType::Function, |compiler| {
        for arg in args {
            declare_variable(compiler, arg)?;
            define_variable(compiler, arg);
        }

        compile_block(compiler, block)?;

        compiler.add_instruction(Instruction::Nil);
        compiler.add_instruction(Instruction::Return);
        Ok(())
    })?;

    let function = Function {
        name: identifier.into(),
        chunk_index,
        arity: args.len(),
    };

    let closure = Closure {
        function,
        upvalues: upvalues,
    };

    let constant = compiler.add_constant(Constant::Closure(closure));
    compiler.add_instruction(Instruction::Closure(constant));

    define_variable(compiler, identifier);

    Ok(())
}

fn compile_while(compiler: &mut Compiler, condition: &Expr, body: &Stmt) -> Result<(), CompilerError> {
    let loop_start = compiler.instruction_index();
    compile_expr(compiler, condition)?;
    let end_jump = compiler.add_instruction(Instruction::JumpIfFalse(0));
    compiler.add_instruction(Instruction::Pop);
    compile_stmt(compiler, body)?;
    let loop_jump = compiler.add_instruction(Instruction::Jump(0));
    compiler.patch_instruction_to(loop_jump, loop_start);
    compiler.patch_instruction(end_jump);
    compiler.add_instruction(Instruction::Pop);
    Ok(())
}

fn compile_if<S: AsRef<Stmt>>(compiler: &mut Compiler, condition: &Expr, then_stmt: &Stmt, else_stmt: Option<S>) -> Result<(), CompilerError> {
    compile_expr(compiler, condition)?;

    let then_index = compiler.add_instruction(Instruction::JumpIfFalse(0));
    compiler.add_instruction(Instruction::Pop);
    compile_stmt(compiler, then_stmt)?;
    
    if let Some(else_stmt) = else_stmt {
        let else_index = compiler.add_instruction(Instruction::Jump(0));
        compiler.patch_instruction(then_index);
        compiler.add_instruction(Instruction::Pop);
        compile_stmt(compiler, else_stmt.as_ref())?;
        compiler.patch_instruction(else_index);
    } else {
        compiler.patch_instruction(then_index);
    }
    Ok(())
}

fn compile_expression_statement(compiler: &mut Compiler, expr: &Expr) -> Result<(), CompilerError> {
    compile_expr(compiler, expr)?;
    compiler.add_instruction(Instruction::Pop);
    Ok(())
}

fn compile_block(compiler: &mut Compiler, ast: &Ast) -> Result<(), CompilerError> {
    compiler.with_scope(|compiler| {
        compile_ast(compiler, ast)
    })
}

fn compile_var_declaration<T: AsRef<Expr>, I: AsRef<str>>(compiler: &mut Compiler, identifier: WithSpan<I>, expr: Option<T>) -> Result<(), CompilerError> {
    declare_variable(compiler, identifier.value.as_ref())?;
    
    //expr
    if let Some(expr) = expr {
        compile_expr(compiler, expr.as_ref())?;
    } else {
        compile_nil(compiler)?;
    }

    define_variable(compiler, identifier.value.as_ref());

    Ok(())
}

fn compile_print(compiler: &mut Compiler, expr: &Expr) -> Result<(), CompilerError> {
    compile_expr(compiler, expr)?;
    compiler.add_instruction(Instruction::Print);
    Ok(())
}

fn compile_expr(compiler: &mut Compiler, expr: &Expr) -> Result<(), CompilerError> {
    match *expr {
        Expr::Number(num) => compile_number(compiler, num),
        Expr::String(ref string) => compile_string(compiler, string),
        Expr::Binary(ref left, operator, ref right) => compile_binary(compiler, operator, left, right),
        Expr::Variable(ref identifier) => compile_variable(compiler, identifier),
        Expr::Nil => compile_nil(compiler),
        Expr::Boolean(boolean) => compile_boolean(compiler, boolean),
        Expr::Assign(ref identifier, ref expr) => compile_assign(compiler, identifier, expr),
        Expr::Logical(ref left, operator, ref right) => compile_logical(compiler, operator, left, right),
        Expr::Call(ref identifier, ref args) => compile_call(compiler, identifier, args),
        Expr::Grouping(ref expr) => compile_expr(compiler, expr),
        Expr::Unary(operator, ref expr) => compile_unary(compiler, operator, expr),
        Expr::Set(ref expr, ref identifier, ref value) => compiler_set(compiler, expr, identifier, value),
        Expr::Get(ref expr, ref identifier) => compiler_get(compiler, expr, identifier),
        ref expr => unimplemented!("{:?}", expr),
    }
}

fn compiler_get(compiler: &mut Compiler, expr: &Expr, identifier: &str) -> Result<(), CompilerError> {
    compile_expr(compiler, expr)?;
    let constant = compiler.add_constant(identifier);
    compiler.add_instruction(Instruction::GetProperty(constant));
    Ok(())
}

fn compiler_set(compiler: &mut Compiler, expr: &Expr, identifier: &str, value: &Expr) -> Result<(), CompilerError> {
    compile_expr(compiler, expr)?;
    compile_expr(compiler, value)?;
    let constant = compiler.add_constant(identifier);
    compiler.add_instruction(Instruction::SetProperty(constant));
    Ok(())
}

fn compile_unary(compiler: &mut Compiler, operator: UnaryOperator, expr: &Expr) -> Result<(), CompilerError> {
    compile_expr(compiler, expr)?;
    match operator {
        UnaryOperator::Minus => compiler.add_instruction(Instruction::Negate),
        UnaryOperator::Bang => compiler.add_instruction(Instruction::Not),
    };

    Ok(())
}

fn compile_call(compiler: &mut Compiler, identifier: &Expr, args: &Vec<Expr>) -> Result<(), CompilerError> {
    compile_expr(compiler, identifier)?;
    for arg in args {
        compile_expr(compiler, arg)?;
    }
    compiler.add_instruction(Instruction::Call(args.len()));
    Ok(())
}

fn compile_logical(compiler: &mut Compiler, operator: LogicalOperator, left: &Expr, right: &Expr) -> Result<(), CompilerError> {
    match operator {
        LogicalOperator::And => compile_logical_and(compiler, left, right),
        LogicalOperator::Or => compile_logical_or(compiler, left, right),
    }
}

//TODO Implement this better, using one less jump, we can easily introduce a JumpIfTrue instruction.
fn compile_logical_or(compiler: &mut Compiler, left: &Expr, right: &Expr) -> Result<(), CompilerError> {
    compile_expr(compiler, left)?;
    let else_jump = compiler.add_instruction(Instruction::JumpIfFalse(0));
    let end_jump = compiler.add_instruction(Instruction::Jump(0));
    compiler.patch_instruction(else_jump);
    compiler.add_instruction(Instruction::Pop);
    compile_expr(compiler, right)?;
    compiler.patch_instruction(end_jump);
    Ok(())
}

fn compile_logical_and(compiler: &mut Compiler, left: &Expr, right: &Expr) -> Result<(), CompilerError> {
    compile_expr(compiler, left)?;
    let end_jump = compiler.add_instruction(Instruction::JumpIfFalse(0));
    compiler.add_instruction(Instruction::Pop);
    compile_expr(compiler, right)?;
    compiler.patch_instruction(end_jump);
    Ok(())
}

fn compile_boolean(compiler: &mut Compiler, boolean: bool) -> Result<(), CompilerError> {
    if boolean {
        compiler.add_instruction(Instruction::True);
    } else {
        compiler.add_instruction(Instruction::False);
    }

    Ok(())
}

fn compile_assign(compiler: &mut Compiler, identifier: &str, expr: &Expr) -> Result<(), CompilerError> {
    compile_expr(compiler, expr)?;
    if let Some(local) = compiler.resolve_local(identifier)? {
        // Local
        compiler.add_instruction(Instruction::SetLocal(local));
    } else if let Some(upvalue) = compiler.resolve_upvalue(identifier)? {
        // Upvalue
        compiler.add_instruction(Instruction::SetUpvalue(upvalue));
    } else {
        // Global
        let constant = compiler.add_constant(identifier);
        compiler.add_instruction(Instruction::SetGlobal(constant));
    }
    Ok(())
}

fn compile_variable(compiler: &mut Compiler, identifier: &str) -> Result<(), CompilerError> {
    if let Some(local) = compiler.resolve_local(identifier)? {
        // Local
        compiler.add_instruction(Instruction::GetLocal(local));
    } else if let Some(upvalue) = compiler.resolve_upvalue(identifier)? {
        // Upvalue
        compiler.add_instruction(Instruction::GetUpvalue(upvalue));
    } else {
        // Global
        let constant = compiler.add_constant(identifier);
        compiler.add_instruction(Instruction::GetGlobal(constant));
    }
    Ok(())
}

fn compile_nil(compiler: &mut Compiler) -> Result<(), CompilerError> {
    compiler.add_instruction(Instruction::Nil);
    Ok(())
}

fn compile_number(compiler: &mut Compiler, num: f64) -> Result<(), CompilerError> {
    let constant = compiler.add_constant(num);
    compiler.add_instruction(Instruction::Constant(constant));
    Ok(())
}

fn compile_string(compiler: &mut Compiler, string: &str) -> Result<(), CompilerError> {
    let constant = compiler.add_constant(string);
    compiler.add_instruction(Instruction::Constant(constant));
    Ok(())
}

fn compile_binary(compiler: &mut Compiler, operator: BinaryOperator, left: &Expr, right: &Expr) -> Result<(), CompilerError> {
    compile_expr(compiler, left)?;
    compile_expr(compiler, right)?;
    match operator {
        BinaryOperator::Plus => compiler.add_instruction(Instruction::Add),
        BinaryOperator::Minus => compiler.add_instruction(Instruction::Subtract),
        BinaryOperator::Less => compiler.add_instruction(Instruction::Less),
        BinaryOperator::LessEqual => { compiler.add_instruction(Instruction::Greater); compiler.add_instruction(Instruction::Not) },
        BinaryOperator::Star => compiler.add_instruction(Instruction::Multiply),
        BinaryOperator::EqualEqual => compiler.add_instruction(Instruction::Equal),
        BinaryOperator::BangEqual => { compiler.add_instruction(Instruction::Equal); compiler.add_instruction(Instruction::Not) },
        BinaryOperator::Greater => compiler.add_instruction(Instruction::Greater),
        BinaryOperator::GreaterEqual => { compiler.add_instruction(Instruction::Less); compiler.add_instruction(Instruction::Not) },
        BinaryOperator::Slash => compiler.add_instruction(Instruction::Divide),
    };
    Ok(())
}
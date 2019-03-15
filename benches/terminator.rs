#[macro_use]
extern crate criterion;

use criterion::Criterion;
use ion_shell::parser::Terminator;

const TEXT: &str = r#"fn execute_bf program
    let program_counter: int = 0

    let memory: hmap[int] = []
    let memory_pointer: int = 0

    let stack: hmap[int] = []
    let stack_pointer: int = 0

    let jumps: hmap[int] = []

    let jumps_program_counter: int = 0
    while test $jumps_program_counter -lt $len($program)
        if test $program[$jumps_program_counter] = "["
            let search_depth: int = 0
            let jump_target: int = $jumps_program_counter
            while test $search_depth -ge 0
                let jump_target += 1
                test $jump_target -ge $len($program) && exit

                match $program[$jump_target]
                    case "["; let search_depth += 1
                    case "]"; let search_depth -= 1
                end
            end
            let jumps[$jumps_program_counter] = $jump_target
        end
        let jumps_program_counter += 1
    end

    let memory[0] = 0
    while test $program_counter -lt $len($program)
        match $program[$program_counter]
            case "+"
                if test @memory[$memory_pointer] -eq 255
                    let memory[$memory_pointer] = 0
                else
                    let memory_value = @memory[$memory_pointer]
                    let memory[$memory_pointer] = $((1 + memory_value))
                    # TODO: let memory[$memory_pointer] += 1
                end
            case "-"
                if test @memory[$memory_pointer] -eq 0
                    let memory[$memory_pointer] = 255
                else
                    let memory_value = @memory[$memory_pointer]
                    let memory[$memory_pointer] = $((-1 + memory_value))
                    # TODO: let memory[$memory_pointer] -= 1
                end
            case ">"
                let memory_pointer += 1
                if test @memory[$memory_pointer] = ""
                    let memory[$memory_pointer] = 0
                end
            case "<"
                let memory_pointer -= 1
                if test @memory[$memory_pointer] = ""
                    let memory[$memory_pointer] = 0
                end
            case "["
                let memory_value = @memory[$memory_pointer]
                if test $memory_value -ne 0
                    let stack[$stack_pointer] = $program_counter
                    let stack_pointer += 1
                else
                    let program_counter = @jumps[$program_counter]
                end
            case "]"
                let stack_pointer -= 1
                let program_counter = @stack[$stack_pointer]
                let program_counter -= 1
            case "."
                # TODO: Nicer decimal to ASCII conversion
                printf \\$(printf '%03o' @memory[$memory_pointer])
            case ","
                echo Input not supported yet
        end

        let program_counter += 1
    end
end

if test @args[1] != ""
    execute_bf @args[1]
else
    execute_bf '-[------->+<]>.++++++.-.++[----->++<]>.>-[--->+<]>--.----------\
        -.---.+++++++..>++++++++++.'

    execute_bf '++++[++++>---<]>-.>-[--->+<]>---.--[->++++<]>+.++++++++.+++++.-\
        -------.>-[--->+<]>.-[----->+<]>-.++++++++.---[->++++<]>.++[->++<]>.+.+\
        +++++++.++.>++++++++++.'

    # https://github.com/brain-lang/brainfuck/blob/master/examples/sierpinski.bf
    execute_bf '[.--..]++++++++[>+>++++<<-]>++>>+<[-[>>+<<-]+>>]>+[-<<<[->[+[-]\
        +>++>>>-<<]<[<]>>++++++[<<+++++>>-]+<<++.[-]<<]>.>+[>>]>+][.]'

    # https://www.rosettacode.org/wiki/99_Bottles_of_Beer/EsoLang
    execute_bf '>+++++++++[<+++++++++++>-]<[>[-]>[-]<<[>+>+<<-]>>[<<+>>-]>>>[-]\
        <<<+++++++++<[>>>+<<[>+>[-]<<-]>[<+>-]>[<<++++++++++>>>+<-]<<-<-]++++++\
        +++>[<->-]>>+>[<[-]<<+>>>-]>[-]+<<[>+>-<<-]<<<[>>+>+<<<-]>>>[<<<+>>>-]>\
        [<+>-]<<-[>[-]<[-]]>>+<[>[-]<-]<++++++++[<++++++<++++++>>-]>>>[>+>+<<-]\
        >>[<<+>>-]<[<<<<<.>>>>>-]<<<<<<.>>[-]>[-]++++[<++++++++>-]<.>++++[<++++\
        ++++>-]<++.>+++++[<+++++++++>-]<.><+++++..--------.-------.>>[>>+>+<<<-\
        ]>>>[<<<+>>>-]<[<<<<++++++++++++++.>>>>-]<<<<[-]>++++[<++++++++>-]<.>++\
        +++++++[<+++++++++>-]<--.---------.>+++++++[<---------->-]<.>++++++[<++\
        +++++++++>-]<.+++..+++++++++++++.>++++++++[<---------->-]<--.>+++++++++\
        [<+++++++++>-]<--.-.>++++++++[<---------->-]<++.>++++++++[<++++++++++>-\
        ]<++++.------------.---.>+++++++[<---------->-]<+.>++++++++[<++++++++++\
        +>-]<-.>++[<----------->-]<.+++++++++++..>+++++++++[<---------->-]<----\
        -.---.>>>[>+>+<<-]>>[<<+>>-]<[<<<<<.>>>>>-]<<<<<<.>>>++++[<++++++>-]<--\
        .>++++[<++++++++>-]<++.>+++++[<+++++++++>-]<.><+++++..--------.-------.\
        >>[>>+>+<<<-]>>>[<<<+>>>-]<[<<<<++++++++++++++.>>>>-]<<<<[-]>++++[<++++\
        ++++>-]<.>+++++++++[<+++++++++>-]<--.---------.>+++++++[<---------->-]<\
        .>++++++[<+++++++++++>-]<.+++..+++++++++++++.>++++++++++[<---------->-]\
        <-.---.>+++++++[<++++++++++>-]<++++.+++++++++++++.++++++++++.------.>++\
        +++++[<---------->-]<+.>++++++++[<++++++++++>-]<-.-.---------.>+++++++[\
        <---------->-]<+.>+++++++[<++++++++++>-]<--.+++++++++++.++++++++.------\
        ---.>++++++++[<---------->-]<++.>+++++[<+++++++++++++>-]<.+++++++++++++\
        .----------.>+++++++[<---------->-]<++.>++++++++[<++++++++++>-]<.>+++[<\
        ----->-]<.>+++[<++++++>-]<..>+++++++++[<--------->-]<--.>+++++++[<+++++\
        +++++>-]<+++.+++++++++++.>++++++++[<----------->-]<++++.>+++++[<+++++++\
        ++++++>-]<.>+++[<++++++>-]<-.---.++++++.-------.----------.>++++++++[<-\
        ---------->-]<+.---.[-]<<<->[-]>[-]<<[>+>+<<-]>>[<<+>>-]>>>[-]<<<++++++\
        +++<[>>>+<<[>+>[-]<<-]>[<+>-]>[<<++++++++++>>>+<-]<<-<-]+++++++++>[<->-\
        ]>>+>[<[-]<<+>>>-]>[-]+<<[>+>-<<-]<<<[>>+>+<<<-]>>>[<<<+>>>-]<>>[<+>-]<\
        <-[>[-]<[-]]>>+<[>[-]<-]<++++++++[<++++++<++++++>>-]>>>[>+>+<<-]>>[<<+>\
        >-]<[<<<<<.>>>>>-]<<<<<<.>>[-]>[-]++++[<++++++++>-]<.>++++[<++++++++>-]\
        <++.>+++++[<+++++++++>-]<.><+++++..--------.-------.>>[>>+>+<<<-]>>>[<<\
        <+>>>-]<[<<<<++++++++++++++.>>>>-]<<<<[-]>++++[<++++++++>-]<.>+++++++++\
        [<+++++++++>-]<--.---------.>+++++++[<---------->-]<.>++++++[<+++++++++\
        ++>-]<.+++..+++++++++++++.>++++++++[<---------->-]<--.>+++++++++[<+++++\
        ++++>-]<--.-.>++++++++[<---------->-]<++.>++++++++[<++++++++++>-]<++++.\
        ------------.---.>+++++++[<---------->-]<+.>++++++++[<+++++++++++>-]<-.\
        >++[<----------->-]<.+++++++++++..>+++++++++[<---------->-]<-----.---.+\
        ++.---.[-]<<<]'
end
"#;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("terminator", |b| b.iter(|| {
        let mut lines = TEXT.lines().peekable();
        while let Some(command) = lines.next() {
            if !command.is_empty() {
                let mut buffer = Terminator::new(command.to_string());
                while !buffer.is_terminated() {
                    if let Some(command) = lines.next() {
                        if !command.starts_with('#') {
                            buffer.append(&command);
                        }
                    }
                }

                println!("{:?}", buffer.consume());
            }
        }
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

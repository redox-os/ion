# Namespaces (colors, scopes and environment variables)
Various functionalities are exposed via namespaces. They are currently colors, scopes and environment variables.

## Syntax
To access namespaces, simply use `${namespace::variable}`.

## Colors (c/color namespace)
Ion aims to make it easy to make your own prompts, without having to use an external script because of its length.
One of the features that make this goal possible is a simple but powerful integration of colors via variables.

Colors available are:
  - black
  - blue
  - cyan
  - dark\_gray
  - default
  - green
  - magenta
  - red
  - yellow
  - light\_blue
  - light\_cyan
  - light\_gray
  - light\_green
  - light\_magenta
  - light\_red
  - light\_yellow

To change the background color, simply append bg to the color (ex: `${c::black}` => `${c::blackbg}`)

Attributes for the command line are also available:
 - blink
 - bold
 - dim
 - hidden
 - reverse
 - underlined

You can also access the full 256 colors using hex or decimal representation. For example ${c::F} accesses the 16th color, ${c::0xFF} the 255th, and ${c::14} would use color #14.

Lastly, you can use true colors using hexes. ${c::0x000000} and ${c::0x000} would print pure black independent of the terminal's color scheme. It should be advised to avoid using those colors except specific use cases where the exact color is required.

As a last tip, you can delimit different attributes using commas, so ${c::black}${c::redbg} is also ${c::black,redbg}.

### Example
```
fn PROMPT
  printf "${c::red}${c::bluebg}${c::bold}%s${c::reset}" $(git branch)
end
```
would print the git branches in bold red over a blue background.

## Scopes (super and global namespaces)
Since Ion has proper scoping contrary to other shells, helpers are provided to access variables in various scopes. The super namespaces crosses a function boundary and the global namespace accesses, well, the global scope.

### Example
```
let a = 1

fn demo
  let b = 2

  fn nested
    echo ${super::b}
    echo ${global::a}
  end
  nested
end

demo
```
Will output 2 and 1

## Environment variable (env namespace)
Ion errors when users access undefined variables. Usually, though, environment variables can't be predicted. It is also clearer to define where they are used. As such, the env namespace will simply emit an empty string if the environment variable is not defined.

### Example
```
echo ${env::SHELL}
```
would output /usr/local/bin/ion on a system with a locally built Ion as login shell.

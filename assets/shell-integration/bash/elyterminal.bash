# ELYTerminal: load the usual startup files, report folder and title, mark prompts and commands.
if [ -n "${ELY_BASH_LOGIN-}" ]; then
  builtin unset ELY_BASH_LOGIN
  [ -r /etc/profile ] && builtin source /etc/profile
  for _ely_file in ~/.bash_profile ~/.bash_login ~/.profile; do
    if [ -r "$_ely_file" ]; then
      builtin source "$_ely_file"
      break
    fi
  done
  builtin unset _ely_file
else
  [ -r ~/.bashrc ] && builtin source ~/.bashrc
fi
_ely_ran=
_ely_armed=
_ely_ps0=
if (( BASH_VERSINFO[0] * 100 + BASH_VERSINFO[1] >= 404 )); then
  # PS0 runs in the shell, so its subscript can note that a command ran.
  _ely_ps0='${_ely_note[_ely_ran=1]-}\e]133;C\a'
elif [ -z "$(builtin trap -p DEBUG)" ]; then
  _ely_debug() {
    [ -n "$_ely_armed" ] || builtin return 0
    case $BASH_COMMAND in _ely_*) builtin return 0 ;; esac
    _ely_armed=
    _ely_ran=1
    builtin printf '\e]133;C\a'
  }
  builtin trap '_ely_debug' DEBUG
fi
_ely_prompt() {
  builtin local ret=$? LC_ALL=C char url= i title
  _ely_armed=
  if [ -n "$_ely_ran" ]; then
    builtin printf '\e]133;D;%s\a' "$ret"
    _ely_ran=
  fi
  for (( i = 0; i < ${#PWD}; i++ )); do
    char=${PWD:i:1}
    case $char in
      [[:cntrl:]]|%) builtin printf -v char '%%%02X' "'$char" ;;
    esac
    url+=$char
  done
  title=${PWD##*/}
  builtin printf '\e]7;file://%s\a\e]2;%s\a' "$url" "${title//[[:cntrl:]]/}"
  builtin return $ret
}
# Last, after anything that sets PS1 or PS0.
_ely_mark() {
  [[ $PS1 == *'\e]133;A'* ]] || PS1='\[\e]133;A\a\]'"$PS1"'\[\e]133;B\a\]'
  if [ -n "$_ely_ps0" ] && builtin shopt -q promptvars && [[ ${PS0-} != *"$_ely_ps0"* ]]; then
    PS0=$_ely_ps0${PS0-}
  fi
  _ely_armed=1
}
PROMPT_COMMAND="_ely_prompt${PROMPT_COMMAND:+;$PROMPT_COMMAND};_ely_mark"

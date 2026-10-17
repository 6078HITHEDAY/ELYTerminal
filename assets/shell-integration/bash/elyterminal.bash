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
_ely_sent=
_ely_n=0
_ely_armed=
_ely_ps0=
if (( BASH_VERSINFO[0] * 100 + BASH_VERSINFO[1] >= 404 )); then
  # PS0 runs in the shell, so its subscript can note that a command ran.
  _ely_ps0='${_ely_note[_ely_ran=1]-}\e]133;C\a'
else
  # bash 3.2 has no PS0: the DEBUG trap sends C, then runs the user's own trap with $? and $_ kept.
  _ely_debug() {
    builtin local status=$?
    if [ -n "$_ely_armed" ] && [[ $BASH_COMMAND != _ely_* ]]; then
      _ely_armed=
      _ely_ran=1
      builtin printf '\e]133;C\a'
    fi
    builtin return $status
  }
  builtin eval "_ely_trap=($(builtin trap -p DEBUG))"
  builtin trap "_ely_debug \"\$_\"${_ely_trap[2]:+; ${_ely_trap[2]}}" DEBUG
  builtin unset _ely_trap
fi
_ely_prompt() {
  builtin local ret=$? LC_ALL=C char url= i title
  _ely_armed=
  _ely_sent=$_ely_ran
  if [ -n "$_ely_ran" ]; then
    builtin printf '\e]133;D;%s\a' "$ret"
    _ely_ran=
  fi
  _ely_d=([1]=$'\e]133;D;'"$ret"$'\a')
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
  if [[ $PS1 != *'\e]133;A'* ]]; then
    if builtin shopt -q promptvars; then
      # bash 3.2 traps no line that is only a subshell; \# counts it, so the prompt sends its D.
      PS1='\[${_ely_d[!_ely_sent && _ely_n && \# != _ely_n]-}${_ely_note[_ely_n = \#]-}\e]133;A\a\]'"$PS1"'\[\e]133;B\a\]'
    else
      PS1='\[\e]133;A\a\]'"$PS1"'\[\e]133;B\a\]'
    fi
  fi
  if [ -n "$_ely_ps0" ] && builtin shopt -q promptvars && [[ ${PS0-} != *"$_ely_ps0"* ]]; then
    PS0=$_ely_ps0${PS0-}
  fi
  _ely_armed=1
}
PROMPT_COMMAND="_ely_prompt${PROMPT_COMMAND:+;$PROMPT_COMMAND};_ely_mark"

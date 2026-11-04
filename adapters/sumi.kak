declare-option -hidden str skk_newline '
'
# Experimental Insert-mode adapter. skk must be on PATH.
declare-option -hidden str skk_directory ''
declare-option -hidden str skk_client ''
define-command -override skk-start %{
    evaluate-commands %sh{
        umask 077
        directory=$(mktemp -d "${TMPDIR:-/tmp}/sumi-kak.XXXXXXXX") || exit
        mkfifo "$directory/keys" "$directory/notify" || exit
        skk kakoune "$directory" </dev/null >"$directory/log" 2>&1 &
        printf "set-option window skk_directory '%s'\n" "$directory"
        printf "set-option window skk_client '%s'\n" "$kak_client"
        printf "evaluate-commands -draft %%{ edit -fifo '%s/notify' '*skk-%s*'; hook buffer BufReadFifo .* %%{ evaluate-commands -client '%s' %%{ evaluate-commands %%file{%s/reply}; echo -to-file '%s/keys' -- __ACK__ %%opt{skk_newline} } } }\n" "$directory" "$kak_client" "$kak_client" "$directory" "$directory"
        for key in a b c d e f g h i j k l m n o p q r s t u v w x y z 0 1 2 3 4 5 6 7 8 9 A B C D E F G H I J K L M N O P Q R S T U V W X Y Z; do
            printf "map window insert %s '<a-;>:echo -to-file %s/keys -- %s %%opt{skk_newline}<ret>'\n" "$key" "$directory" "$key"
        done
        for pair in '<space>:Space' '<ret>:Enter' '<backspace>:Backspace' '<c-j>:C-j' '<c-g>:C-g' '<minus>:-' ',:,' '.:.' '<quote>:Apostrophe'; do
            key=${pair%:*}; event=${pair#*:}
            printf "map window insert %s '<a-;>:echo -to-file %s/keys -- %s %%opt{skk_newline}<ret>'\n" "$key" "$directory" "$event"
        done
    }
    map window insert <esc> '<a-;>:skk-stop<ret>'
    map window insert <c-k> '<a-;>:skk-stop<ret>'
    map window insert <left> '<a-;>:skk-stop<ret>'
    map window insert <right> '<a-;>:skk-stop<ret>'
    map window insert <up> '<a-;>:skk-stop<ret>'
    map window insert <down> '<a-;>:skk-stop<ret>'
    echo 'SKK on (Esc ends Insert mode)'
}
define-command -override skk-stop %{
    echo -to-file "%opt{skk_directory}/keys" -- __FINISH__ %opt{skk_newline}
}
define-command -override -hidden skk-cleanup %{ 
    remove-hooks window skk
    evaluate-commands %sh{
        [ -n "$kak_opt_skk_directory" ] || exit
        for key in a b c d e f g h i j k l m n o p q r s t u v w x y z 0 1 2 3 4 5 6 7 8 9 A B C D E F G H I J K L M N O P Q R S T U V W X Y Z '<space>' '<ret>' '<backspace>' '<c-j>' '<c-g>' '<esc>' '<c-k>' '<minus>' ',' '.' '<quote>' '<left>' '<right>' '<up>' '<down>'; do
            printf 'unmap window insert %s\n' "$key"
        done
        printf "try %%{ delete-buffer! '*skk-%s*' }\n" "$kak_client"
    }
    set-option window skk_directory ''
    echo 'SKK off'
}
map global insert <c-k> '<a-;>:skk-start<ret>'

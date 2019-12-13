#!/bin/sh

vim_home="/home/stephen/.vim"
nvim_home="/home/stephen/.config/nvim"
if [[ -d $vim_home ]]; then
    if [[ -L $nvim_home ]]; then
	rm -f $nvim_home
    fi
    ln -sf $vim_home $nvim_home
fi

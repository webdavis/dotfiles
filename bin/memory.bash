#!/usr/bin/env bash

# To stress test memory on a Linux box, run:
#
#   stress-ng --vm 2 --vm-bytes 1G --timeout 60s
#

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

TYPE="${BLOCK_INSTANCE:-mem}"

awk -v type=$TYPE '
    /^MemTotal:/ { total_memory=$2 }
    /^MemFree:/ { free_memory=$2 }
    /^Buffers:/ { free_memory+=$2 }
    /^Cached:/ { free_memory+=$2 }
    /^SwapTotal:/ { total_swap=$2 }
    /^SwapFree:/ { free_swap=$2 }
    END {
	if (type == "swap") {
	    free=free_swap/1024/1024
	    used=(total_swap-free_swap)/1024/1024
	    total=total_swap/1024/1024
	} else {
	    free=free_memory/1024/1024
	    used=(total_memory-free_memory)/1024/1024
	    total=total_memory/1024/1024
	}

	percent=0
	if (total > 0) {
		percent=used/total*100
	}

	# Fulltext.
	printf("%.1f G\n", used)

	# Color.
	if (percent > 90) {
	    print("#FF0000\n")
	} else if (percent > 80) {
	    print("#FFAE00\n")
	} else if (percent > 70) {
	    print("#FFF600\n")
	}
    }
' /proc/meminfo

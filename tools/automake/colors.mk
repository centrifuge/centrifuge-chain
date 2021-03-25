###############################################################################
# Centrifuge                                                                  #
# Cash on Steroids                                                            #
#                                                                             #
# tools/automake/colors.mk                                                    #
#                                                                             #
# Handcrafted since 2020 by Centrifuge tribe                                  #
# All rights reserved                                                         #
#                                                                             #
#                                                                             #
# Description: Shell colors definition.                                       #
###############################################################################


# Default background color definition
COLOR_BACK=\033[49m

# Foreground colors definition
COLOR_RED=\033[0;31m$(COLOR_BACK)
COLOR_GREEN=\033[38;5;112m$(COLOR_BACK)
COLOR_BLUE=\033[38;5;33m$(COLOR_BACK)
COLOR_YELLOW=\033[0;33m$(COLOR_BACK)
COLOR_ORANGE=\033[38;5;166m$(COLOR_BACK)
COLOR_WHITE=\033[97m$(COLOR_BACK)
COLOR_PINK=\033[35;40m$(COLOR_BACK)
COLOR_RESET=\033[0m
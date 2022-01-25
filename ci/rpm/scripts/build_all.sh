
   
#!/bin/bash

# Enable devtools if variable is set and not empty
if [ -n "$DEVTOOLS" ]; then
    source /opt/rh/devtoolset-7/enable
fi

/bin/build_spec $HOME/rpmbuilder/rpmbuild/SPECS/*.spec
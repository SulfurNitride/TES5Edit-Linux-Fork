cmake_minimum_required(VERSION 3.10)
project(xedit_nifly_wrapper)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Path to the nifly source tree (already built as static lib)
set(NIFLY_SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../nifly")
set(NIFLY_BUILD_DIR "${NIFLY_SOURCE_DIR}/build")

# Build the shared wrapper library
add_library(nifly_wrapper SHARED
    xedit_nifly_wrapper.cpp
)

# Include nifly headers
target_include_directories(nifly_wrapper PRIVATE
    ${NIFLY_SOURCE_DIR}/include
    ${NIFLY_SOURCE_DIR}/external
)

# Link the pre-built static nifly library
if(MSVC)
    # MSVC: nifly.lib in a config subdirectory (e.g., Release/)
    find_library(NIFLY_LIB nifly PATHS
        ${NIFLY_BUILD_DIR}/src/Release
        ${NIFLY_BUILD_DIR}/src/RelWithDebInfo
        ${NIFLY_BUILD_DIR}/src/Debug
        ${NIFLY_BUILD_DIR}/src
        NO_DEFAULT_PATH
    )
    if(NOT NIFLY_LIB)
        set(NIFLY_LIB ${NIFLY_BUILD_DIR}/src/Release/nifly.lib)
    endif()
    target_link_libraries(nifly_wrapper PRIVATE ${NIFLY_LIB})
else()
    target_link_libraries(nifly_wrapper PRIVATE
        ${NIFLY_BUILD_DIR}/src/libnifly.a
    )
endif()

# On Linux, set symbol visibility and ensure all nifly symbols are pulled in
if(UNIX)
    set_target_properties(nifly_wrapper PROPERTIES
        C_VISIBILITY_PRESET hidden
        CXX_VISIBILITY_PRESET hidden
        VISIBILITY_INLINES_HIDDEN ON
    )
    # Position-independent code is required for shared libs linking static libs
    target_compile_options(nifly_wrapper PRIVATE -fPIC)
endif()

# Output name: libnifly_wrapper.so (Linux) or nifly_wrapper.dll (Windows)
if(WIN32)
    set_target_properties(nifly_wrapper PROPERTIES
        OUTPUT_NAME nifly_wrapper
    )
else()
    set_target_properties(nifly_wrapper PROPERTIES
        OUTPUT_NAME nifly_wrapper
        PREFIX "lib"
    )
endif()

#ifndef STRING_BUFFER_H
#define STRING_BUFFER_H

#include <cstdint>
#include <functional>
#include <QString>

/// Call an FFI function whose last two parameters are (char* buf, int32_t buf_len)
/// and return the result as a QString.  Returns an empty QString on error.
///
/// Usage with raw function pointers:
///   ffiString(ffi.xedit_plugin_filename, pluginIdx);
///
/// Usage with lambdas (for multi-arg FFI calls):
///   ffiString([&](char* buf, int len) { return ffi.xedit_group_name(p, g, buf, len); });
///
template <typename Fn, typename... Args>
inline QString ffiString(Fn fn, Args... args)
{
    char buf[4096];
    int32_t result = fn(args..., buf, static_cast<int32_t>(sizeof(buf)));
    if (result < 0)
        return {};
    return QString::fromUtf8(buf, result);
}

/// Overload for lambdas/callables that take (char*, int32_t) directly
inline QString ffiString(const std::function<int32_t(char*, int32_t)>& fn)
{
    char buf[4096];
    int32_t result = fn(buf, static_cast<int32_t>(sizeof(buf)));
    if (result < 0)
        return {};
    return QString::fromUtf8(buf, result);
}

#endif // STRING_BUFFER_H

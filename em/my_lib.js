mergeInto(LibraryManager.library, {
    block_on_promise__deps: ['$EmterpreterAsync'],
    block_on_promise: function(handle) {
        var promise = Module.STDWEB_PRIVATE.acquire_js_reference(handle);
        EmterpreterAsync.handle(function(resume) {
            promise.then(function() {
                resume();
            });
        });
    }
});
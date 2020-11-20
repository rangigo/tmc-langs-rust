(function() {var implementors = {};
implementors["chrono"] = [{"text":"impl AddAssign&lt;Duration&gt; for NaiveDate","synthetic":false,"types":[]},{"text":"impl AddAssign&lt;Duration&gt; for NaiveDateTime","synthetic":false,"types":[]},{"text":"impl AddAssign&lt;Duration&gt; for NaiveTime","synthetic":false,"types":[]}];
implementors["num_rational"] = [{"text":"impl&lt;T:&nbsp;Clone + Integer + NumAssign&gt; AddAssign&lt;Ratio&lt;T&gt;&gt; for Ratio&lt;T&gt;","synthetic":false,"types":[]},{"text":"impl&lt;T:&nbsp;Clone + Integer + NumAssign&gt; AddAssign&lt;T&gt; for Ratio&lt;T&gt;","synthetic":false,"types":[]},{"text":"impl&lt;'a, T:&nbsp;Clone + Integer + NumAssign&gt; AddAssign&lt;&amp;'a Ratio&lt;T&gt;&gt; for Ratio&lt;T&gt;","synthetic":false,"types":[]},{"text":"impl&lt;'a, T:&nbsp;Clone + Integer + NumAssign&gt; AddAssign&lt;&amp;'a T&gt; for Ratio&lt;T&gt;","synthetic":false,"types":[]}];
implementors["tokio"] = [{"text":"impl AddAssign&lt;Duration&gt; for Instant","synthetic":false,"types":[]}];
implementors["uom"] = [{"text":"impl&lt;Ul:&nbsp;?Sized, Ur:&nbsp;?Sized, V&gt; AddAssign&lt;Quantity&lt;dyn Dimension&lt;J = Z0, Kind = dyn Kind + 'static, I = Z0, L = Z0, M = Z0, Th = PInt&lt;UInt&lt;UTerm, B1&gt;&gt;, T = Z0, N = Z0&gt; + 'static, Ur, V&gt;&gt; for ThermodynamicTemperature&lt;Ul, V&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;Ul: Units&lt;V&gt;,<br>&nbsp;&nbsp;&nbsp;&nbsp;Ur: Units&lt;V&gt;,<br>&nbsp;&nbsp;&nbsp;&nbsp;V: Num + Conversion&lt;V&gt; + AddAssign&lt;V&gt;,&nbsp;</span>","synthetic":false,"types":[]},{"text":"impl&lt;D:&nbsp;?Sized, Ul:&nbsp;?Sized, Ur:&nbsp;?Sized, V&gt; AddAssign&lt;Quantity&lt;D, Ur, V&gt;&gt; for Quantity&lt;D, Ul, V&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;D: Dimension,<br>&nbsp;&nbsp;&nbsp;&nbsp;D::Kind: AddAssign,<br>&nbsp;&nbsp;&nbsp;&nbsp;Ul: Units&lt;V&gt;,<br>&nbsp;&nbsp;&nbsp;&nbsp;Ur: Units&lt;V&gt;,<br>&nbsp;&nbsp;&nbsp;&nbsp;V: Num + Conversion&lt;V&gt; + AddAssign&lt;V&gt;,&nbsp;</span>","synthetic":false,"types":[]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()
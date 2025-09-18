package net.obscura.vpnclientapp.activities

import android.os.Bundle
import android.view.ViewGroup
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import androidx.appcompat.app.AppCompatActivity
import net.obscura.vpnclientapp.ui.ObscuraWebView

class MainActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        setContentView(ObscuraWebView(this),
            ViewGroup.LayoutParams(MATCH_PARENT, MATCH_PARENT)
        )
    }
}
